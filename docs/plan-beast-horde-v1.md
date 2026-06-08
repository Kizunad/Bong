# Bong · plan-beast-horde-v1 · 骨架

**兽潮（大迁徙）生态事件**——实装 worldview §七 生态联动中的"大迁徙"机制：当某区域灵气被吸干临近死域时，该 zone 内所有野生生物疯狂向最近正数灵气区迁移，形成"兽潮"。玩家可以逆着兽潮走找到即将干涸的死域、或顺着走遭遇领地争夺战。大规模实体迁移使用 Flow Field（流场寻路），避免百只野兽同时跑个体 A* 拖垮 20 TPS。

## 目标

- 实装 worldview §七「大迁徙」：zone 灵气耗尽触发野兽集体迁移，玩家可逆向定位将涸死域 / 顺向遭遇领地争夺战
- 工程目标：用 Flow Field 流场寻路支撑百只级野兽并发迁移（迁移子系统 ≤ 5ms/tick），不拖垮 20 TPS
- 形成生态链式反应：兽潮加速目标 zone 灵气消耗，可能引发相邻 zone 连锁塌缩
- 验收：`ZoneDepletionEvent → BeastHordeEvent` e2e 打通，负压灭杀 + 领地争夺 + agent 叙事闭环

**来源**：worldview §七 生态联动 §"大迁徙" + docs/scribble.md §"流场寻路 (Flow Field)" 技术方案

**前置条件**：
- `plan-qi-physics-v1` ✅ — zone spirit_qi 实时值 + `ZoneDepletionEvent`（spirit_qi 接近 0 的 zone 广播）
- `plan-fauna-v1` ✅ — `FaunaKind` 注册表 + 野兽 entity 组件体系
- `plan-npc-ai-v1` ✅ — big-brain Scorer/Action 框架 + `AsyncComputeTaskPool` 寻路基础设施
- `plan-world-ecology-events-v1` ✅ — 世界生态事件框架（ZoneEvent 生命周期管理）

**交叉引用**：`plan-faction-wars-v1` ⬜（兽潮 + 派系战争可能叠加：兽潮冲进战场 = 野生干扰）· `plan-npc-virtualize-v3` ⬜（dormant 野兽迁移不需要完整 AI，但 dormant 数量要随兽潮 zone 转移同步调整）· `plan-npc-perf-v1` ✅（大批量 NPC 优化基础；流场寻路是此 plan 的高并发解法）

**worldview 锚点**：
- **§七:750 生态联动 大迁徙**："大区域灵气被吸干即将化为死域时，所有野生生物疯狂向附近正数灵气区狂奔，形成'兽潮'。逆着兽潮走 = 找到即将干涸的死域；顺着走 = 遭遇领地争夺战"
- **§七:751 负压灭杀**："野兽被击退进负数区 → 材质瞬间枯萎化为飞灰"——兽潮走向错了会触发此效果
- **§二 压强法则**：灵气从高浓度流向低浓度，野兽的迁移方向与灵气流向一致（生物向高浓度区跑，逃离低浓度区）
- **§七:721 噬元鼠群**：兽潮主力之一——打坐的修士等于"指路牌"，散发真元波动吸引迁徙鼠群

**qi_physics 锚点**：
- 兽潮本身不产生灵气，只是存量的空间转移；zone spirit_qi 继续由 `qi_physics` 管理
- 野兽在迁徙中死亡（被击退进负灵域）：`qi_physics::qi_release_to_zone` 归还真元（材质枯萎动画 VFX）
- 兽潮带来的区域 NPC 浓度变化 → zone 灵气吸收速率改变（更多生物吸收 = 目标 zone 灵气加速消耗）——这是链式反应：一个 zone 塌 → 兽潮冲进相邻 zone → 相邻 zone 灵气也加速消耗

---

## 接入面 Checklist

- **进料**：`ZoneDepletionEvent { zone, spirit_qi_rate_of_change }`（qi-physics 发出，速率连续负且 spirit_qi < HORDE_TRIGGER_THRESHOLD）+ `FaunaEntityQuery`（zone 内存活野兽 entity 集合）+ zone 邻接关系表（ZoneGraph，目标 zone 选择依据）
- **出料**：`BeastHordeEvent { source_zone, target_zone, beast_count, phase }` + `FlowField { zone: ZoneId, vector_grid: Vec<Vec2> }`（流场向量表）+ `HordeMigrationComponent { target_zone, assigned_flow_field }`（野兽个体目标标记）+ narration event（天道感知到大迁徙触发叙事）
- **共享类型**：复用 `FaunaKind` / `QiTransfer` / `bong:vfx_event`；新增 `BeastHordeEvent` / `FlowField` / `HordeMigrationComponent`
- **跨仓库契约**：server `bong:ecology` channel 广播 `BeastHordeEvent`（agent 消费生成叙事）；client 收到 `bong:horde_vfx` 触发大规模粒子波（尘土 + 兽群轮廓）；agent 可通过叙事引导玩家"某方向有大批野兽迁移"
- **worldview 锚点**：§七 大迁徙 + §七 负压灭杀 + §二 压强法则
- **qi_physics 锚点**：野兽死亡 `qi_release_to_zone` / 兽群聚集加速目标 zone 消耗（不创生灵气，只加速已有消耗速率）

---

## §0 Flow Field 设计（替代大规模个体 A*）

**原则**（来自 scribble.md 技术方案）：百只以上野兽向同一目标迁移时，不跑个体 A*。改用流场：
1. 以"目标 zone 中心"为目标点，对 source_zone 区域运行 **一次** Dijkstra/BFS，生成 `FlowField { zone_id, vector_grid: Vec<Vec2> }`（每格一个方向向量）
2. 所有迁徙野兽在每 tick 读取自身所在格的向量 → 移动；不需要单独寻路
3. CPU 消耗从 `O(N × path_length)` 降为 `O(grid_size + N)`（grid_size 约 1 zone = 256×256 格）
4. 流场放入 `AsyncComputeTaskPool` 异步计算，结果 channel 回 main thread，计算期间野兽维持原地踏步/恐慌动画

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 触发条件 + `BeastHordeEvent` 数据模型 + Flow Field 原型 | 10 单测 green；`ZoneDepletionEvent` → `BeastHordeEvent` e2e |
| **P1** | ⬜ | `FlowField` 计算 + 野兽批量迁移行为 + 性能验收 | **主指标**：FlowField + 野兽迁移子系统 ≤ 5ms/tick（200 兽）；**补充指标（总帧预算）**：server 整帧 tick ≤ 50ms；流场方向向量正确朝目标 zone |
| **P2** | ⬜ | 负压灭杀（误入负灵域 → 枯萎飞灰）+ 领地争夺战 | 野兽进入 spirit_qi<0 区域触发枯萎；两个 zone 兽潮汇聚 → 野兽互殴 |
| **P3** | ⬜ | Agent narration + client 大规模迁移 VFX + dormant 同步 | agent 发出"大批野兽正向东逃窜"类叙事；client 能感知兽潮方向 |

---

## P0 — 触发条件 + 数据模型

- [ ] `HORDE_TRIGGER_THRESHOLD: f64 = 0.05`（spirit_qi 低于此值且连续 N tick 在下降时触发）
- [ ] `BeastHordeEvent { source_zone: ZoneId, target_zone: ZoneId, beast_count: u32, phase: HordePhase, tick: u64 }` （`server/src/ecology/beast_horde.rs`）
- [ ] `HordePhase` enum：`Gathering / Migrating / Dispersed / Annihilated`（野兽被负压消灭）
- [ ] `target_zone` 选择：从 ZoneGraph 找 source_zone 邻接中 spirit_qi 最高的 zone
- [ ] `ZoneDepletionEvent` → `BeastHordeDetectSystem`（每 5s 扫描一次）→ emit `BeastHordeEvent`
- [ ] ≥ 10 单测（触发阈值边界 / target_zone 选择最高 spirit_qi / 已有兽潮中不重复触发 / ZoneDepletionEvent 守恒律传递正确）

---

## P1 — FlowField + 批量迁移 + 性能

- [ ] `FlowField { zone: ZoneId, vectors: Vec<Vec<Vec2>>, computed_tick: u64 }` resource（`server/src/ecology/flow_field.rs`）
- [ ] `FlowFieldComputeTask`（`AsyncComputeTaskPool`）：以 target_zone 入口为目标，BFS source_zone block graph → 每格 Vec2 方向；避开 spirit_qi < 0 的格子
- [ ] `HordeMigrationComponent { target_zone, flow_field_ref }` 附加到迁徙野兽 entity
- [ ] `horde_migration_system`：每 tick 读 `HordeMigrationComponent.flow_field_ref[pos]` → 移动；到达 target_zone 边界 → 移除 component（恢复正常 AI）
- [ ] 性能验收 **主指标**：FlowField + 野兽迁移子系统 ≤ 5ms/tick（200 兽同时迁移，`bevy_diagnostic` 采样子系统耗时）；**补充指标（总帧预算）**：server 整帧 tick ≤ 50ms（20 TPS，本子系统不得挤爆整帧）
- [ ] ≥ 15 单测（flow field 方向正确朝目标 / 障碍物绕行 / 跨 chunk 边界移动 / 多 horde 独立 flow field 互不干扰 / 性能 mock 200 entity 5ms 内）

---

## P2 — 负压灭杀 + 领地争夺

- [ ] 负压灭杀：`horde_negative_pressure_system` 检测迁徙野兽进入 spirit_qi < 0 区域 → emit `FaunaWitherEvent { entity, position }` → 播放枯萎 VFX（`BongSpriteParticle` 颜色 `#808080`，飞灰效果）→ despawn entity + `qi_release_to_zone`
- [ ] 领地争夺：当两个 source_zone 的兽潮同时涌向同一 target_zone → 野兽之间触发 `assign_hostile_encounters`（npc-ai-v1 已有，faction = 不同 source_zone 的野兽视为不同"种群"）
- [ ] ≥ 10 单测（进负灵域触发枯萎守恒律 / 两股兽潮相遇 hostile score 上升 / 枯萎 VFX event 正确 emit）

---

## P3 — Agent 叙事 + client VFX + dormant 同步

- [ ] agent 消费 `bong:ecology` channel 的 `BeastHordeEvent` → 生成叙事（broadcast scope，style: perception）：
  - "南方有大批噬元鼠群正向北迁徙——那片区域的灵气怕是快撑不住了。"
  - "两股兽群在血谷北口碰头，厮打声传来，遮天蔽日。"
- [ ] client VFX：`bong:horde_vfx` CustomPayload → 在 source_zone 方向渲染尘土粒子列（`BongLineParticle`，颜色 `#C8B060`，沿迁移方向流动，interval 2s）
- [ ] dormant 同步：兽潮结束（野兽抵达 target_zone 并 `Dispersed`）→ npc-virtualize dormant SoA 内 source_zone 野兽数量减少 / target_zone 增加（按比例，不要求精确同步）
- [ ] ≥ 5 e2e 测试（agent 收到 event → 叙事发出 / client VFX event 触发 / dormant zone 计数正确变化）

---

## §8 开放问题（P0 决策门收口）

1. **触发阈值 0.05**：是否太低导致频繁触发？或太高导致兽潮出现时 zone 已经是死域（野兽进来也无济于事）——建议 Explore agent 查当前 zone spirit_qi 典型值域
2. **target_zone 选择**：只选最高 spirit_qi 的邻接 zone，还是加权（距离 + spirit_qi）？避免所有兽潮都涌进同一 zone 造成 spirit_qi 雪崩
3. **dormant 野兽是否参与迁徙**：dormant 是轻量化模拟，不需要真实移动；但 dormant 野兽数量应该在 zone 变化时同步调整（zone A 塌 → dormant 从 zone A 移到 zone B），v1 只同步计数，不实际 spawn
4. **flow field 更新频率**：target_zone 被其他 horde 野兽占据后 spirit_qi 下降，flow field 是否需要重新计算（目标变了）？v1 只算一次，v2 按 5min 刷新
5. **噬元鼠群特例**：worldview §七 说打坐散发的真元波动会吸引"区块内所有噬元鼠"——是否应让噬元鼠单独响应玩家打坐事件（不仅仅是 zone 耗尽时），独立于大兽潮机制
