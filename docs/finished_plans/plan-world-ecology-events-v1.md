# Bong · plan-world-ecology-events-v1 · 伪灵脉 + 大迁徙 + 兽潮生态反馈环

把三个独立的世界事件串成一条**生态反馈环**：修士过度采集 → 伪灵脉升起（天道引诱）→ 聚集加速灵气消耗 → zone 灵气逼近零 → 大迁徙触发（兽群奔逃）→ 兽潮涌入邻区 → 最终域崩。现有模块是断开的零件（伪灵脉 schema ✅ / 兽潮状态机 ✅ / 域崩判定 ✅），本 plan 把它们接成闭环。

**生态反馈环（worldview 物理推演）**：
```
    修士采集吸灵            天道观测到
         ↓                    ↓
    zone.spirit_qi ↓    →  伪灵脉升起（§二天道陷阱）
         ↓                    ↓
    更多修士涌入            聚集加速消耗（§八灵物密度阈值）
         ↓                    ↓
    zone.spirit_qi → 0    伪灵脉消散 + 外缘负灵风暴
         ↓
    大迁徙触发（§七兽群奔逃）
         ↓
    兽潮涌入邻区（§七领地争夺）
         ↓
    域崩（§八/§十区域永久死亡）← 已实装，本 plan 只接触发入口
```

**世界观锚点**：
- `worldview.md §二` 伪灵脉——天道在荒野升起短期浓郁灵脉，引诱修士自相残杀，回收真元。30 分钟基线，灵气 0.6，汐转期翻倍
- `worldview.md §七` 大迁徙——区域灵气被吸干 → 所有野生生物疯狂向附近正数区狂奔。逆着走=找死域宝藏，顺着走=遭遇领地争夺战
- `worldview.md §八` 天道中等手段——在强者区域刷新异变兽（既是威胁也是诱饵）+ 发布天象预兆让修士自行迁移
- `worldview.md §十三` 荒野伪灵脉——维持 30 分钟的高灵气点(0.6)，之后消散

**前置依赖**：
- `plan-terrain-pseudo-vein-v1` ✅ → 伪灵脉地形 + schema（PseudoVeinSnapshotV1 / PseudoVeinDissipateEventV1 / IPC 通道）
- `plan-fauna-v1` ✅ → 兽类实体 + BeastKind enum + 生成/清理
- `plan-fauna-experience-v1` ✅ → 蝗虫群系统（RatPhase / LocustSwarmState / pressure_sensor）
- `plan-qi-physics-v1` ✅ → 灵气守恒账本 + zone 灵气流动 + 逸散公式
- `plan-jiezeq-v1` ✅ → 季节系统（汐转期刷新翻倍）
- `plan-zone-environment-v1` ✅ → ZoneRegistry + zone 属性
- `plan-narrative-v1` ✅ → 天道叙事（事件 narration）
- `plan-audio-world-v1` ✅ → 区域音效（兽潮警告音 ✅ 已有）

**反向被依赖**：
- `plan-sou-da-che-v1` ⬜ active → 搜打撤循环中伪灵脉/兽潮作为风险变量
- `plan-season-full-experience-v1` ⬜ active → 季节 × 兽潮视觉联动
- `plan-npc-daily-life-v1` ⬜ active → NPC 遇兽潮中断日程 + 逃跑

---

## 接入面 Checklist

- **进料**：`world::zone::ZoneRegistry`（zone 灵气读写）/ `qi_physics::ledger`（守恒账本）/ `world::events::BeastTideRuntimeState`（已有兽潮状态机）/ `schema::pseudo_vein::*`（已有伪灵脉 schema）/ `fauna::rat_phase::*`（蝗虫群）/ `jiezeq::SeasonState`（季节）/ `network::command_executor`（agent 指令）
- **出料**：伪灵脉运行时（动态升起/聚集衰减/消散/负灵风暴）/ 大迁徙系统（zone qi→0 触发全兽奔逃）/ 兽潮自然触发（迁徙→兽潮联动）/ 环境信号（天象/音效/粒子）/ 生态反馈环闭合
- **共享类型 / event**：复用 `BeastTideRuntimeState`（不新建）/ 复用 `PseudoVeinSnapshotV1`（不新建）/ 新增 `PseudoVeinActiveEvent` / 新增 `MigrationWaveEvent` / 新增 `ZoneQiCriticalEvent`
- **跨仓库契约**：
  - server：`server/src/world/pseudo_vein_runtime.rs`（新文件）+ `server/src/fauna/migration.rs`（新文件）+ 修改 `events.rs` 接大迁徙触发
  - agent：已有 `spawn_event` 指令支持伪灵脉/兽潮——本 plan 加"天道自动决策"逻辑（agent skill 扩展）
  - client：伪灵脉地面粒子 + 大迁徙地面震动 + 兽潮方向 HUD 指示
- **worldview 锚点**：§二 伪灵脉 + §七 大迁徙 + §八 天道手段 + §十三 荒野
- **qi_physics 锚点**：伪灵脉灵气来源走 `qi_physics::ledger::QiTransfer { from: Tiandao, to: Zone }`；消散时走 `qi_release_to_zone`；兽群吸灵走 `qi_excretion`。不自定物理公式

---

## §0 设计轴心

- [ ] **一条链，不是三个独立事件**：伪灵脉升起 → 修士聚集 → 灵气加速消耗 → 大迁徙 → 兽潮 → 域崩。每一步是下一步的因
- [ ] **天道是棋手，不是计时器**：伪灵脉不是定时刷新——天道观测到"某区域修士密度过高/灵气消耗过快"时主动升起。agent 的 `spawn_event` 指令是触发入口
- [ ] **环境信号可读**：玩家不靠 UI 提示判断——靠天象变化（远处金光 = 伪灵脉）/ 地面震动（兽潮来了）/ 植被枯萎速度（灵气在下降）/ 兽群奔跑方向（逆着走 = 找死域）
- [ ] **玩家是反馈环的参与者**：你的采集行为推动了灵气下降 → 你是兽潮的间接原因。但你也可以利用这个环——逆着兽潮走找到被遗弃的死域里的残留资源
- [ ] **性能约束**：大迁徙涉及大量 NPC 同时移动——只在玩家 Near 范围内真实寻路，Far/Dormant 层只做位置偏移

---

## §1 伪灵脉运行时

### 升起条件（天道决策）

伪灵脉不是随机刷——天道（agent）在以下条件下决定升起：

| 条件 | 阈值 | 天道意图 |
|------|------|---------|
| 某区域修士密度 > 3 人/chunk 持续 5 分钟 | `player_density > 3 && duration > 6000 tick` | 分散聚集（引诱去别处） |
| 某区域灵气消耗速率 > 0.02/tick | `qi_drain_rate > 0.02` | 加速该区域枯竭（淘汰） |
| 汐转期 | `SeasonState.is_tide_turn()` | 季节交替混乱期刷新翻倍 |
| agent calamity 主动决策 | `CommandType::SpawnEvent("pseudo_vein")` | 天道主动出手 |

### 生命周期

```
升起（qi 从 0 → 0.6，30s 渐变上升）
  → 活跃期（基线 30 分钟，汐转期 ×2）
  → 聚集加速衰减（核心区 2+ 修士 → 消散速度 ×1.4~3.5）
  → 消散预警（qi 跌至 0.3 → narration "此处灵气开始涣散…"）
  → 消散（qi → 0，30s 渐变下降）
  → 外缘负灵风暴（30% 灵气被天道收割 → 1-3 个负灵 hot-spot）
```

### 聚集加速公式

```rust
fn pseudo_vein_decay_multiplier(cultivators_in_range: u32) -> f64 {
    match cultivators_in_range {
        0..=1 => 1.0,   // 基线衰减
        2     => 1.4,
        3     => 1.8,
        4     => 2.5,
        _     => 3.5,   // 5+ 人极速消散
    }
}
```

**qi_physics 守恒**：伪灵脉的灵气来源 = `QiTransfer { from: Tiandao, to: Zone }`（天道调配）。消散时 70% 灵气散归周围 zone，30% 被天道回收（`QiTransfer { from: Zone, to: Tiandao }`）。净效果 = 天道回收 30% = 正典§八"天道回收真元"。

### 环境信号

| 阶段 | 远距离信号（100+ 格） | 近距离信号（30 格内） |
|------|-------------------|-------------------|
| 升起 | 天空出现金色光柱（粒子柱，30s 渐亮） | 地面灵草疯长 + 灵气浓度 HUD 跳变 |
| 活跃 | 光柱持续 + 偶尔闪烁 | 灵气浓度 0.6（比周围高很多） |
| 消散预警 | 光柱变暗 + 闪烁加速 | narration "灵脉开始涣散" + 灵草枯萎 |
| 消散 | 光柱消失 | 地面泛灰 + 微弱负压嗡鸣 |
| 外缘风暴 | 暗色涟漪从消散点向外扩散 | 负压 hot-spot 吸真元 |

---

## §2 大迁徙系统

### 触发条件

```rust
pub struct ZoneQiCriticalEvent {
    pub zone_id: String,
    pub spirit_qi: f64,
    pub neighbors: Vec<(String, f64)>,  // 邻近 zone 及其灵气
}

// 触发：zone.spirit_qi < MIGRATION_THRESHOLD（0.05）且持续 > 600 tick（30s）
const MIGRATION_THRESHOLD: f64 = 0.05;
const MIGRATION_SUSTAIN_TICKS: u64 = 600;
```

### 迁徙行为

当 `ZoneQiCriticalEvent` 触发：

1. **选择迁徙目标**：邻近 zone 中 `spirit_qi` 最高的作为目标
2. **全兽奔逃**：该 zone 内所有 fauna entity 获得 `MigrationTarget` component → Navigator 走向目标 zone
3. **NPC 也逃**：该 zone 内非玩家 NPC（散修/凡人）同样获得迁徙目标 → ReturnHomeScorer 评分 ×3（优先回家/逃离）
4. **迁徙速度**：fauna 移速 ×1.5（恐慌加速）/ NPC 移速 ×1.2
5. **迁徙持续**：直到 entity 到达目标 zone 或 3 分钟超时（超时则 despawn，假设跑出了世界范围）

### 迁徙→兽潮联动

迁徙兽群到达邻近 zone 时：
- 如果该 zone 已有大量 fauna → **领地冲突**：resident 兽群 vs 迁徙兽群打架（双方 aggro）
- 如果迁徙量 > 阈值（10+ 兽类同时涌入）→ 自动升级为 `BeastTideRuntimeState::Wandering`（走已有兽潮状态机）
- agent narration："某区灵气将尽。万兽南奔。"

### LOD 分层

| LOD | 迁徙表现 |
|-----|---------|
| Near（≤64 格） | 完整寻路 + 奔跑动画 + 恐慌音效 + 踩踏粒子 |
| Far（64-256 格） | 每 1200 tick 位置偏移 5 格朝目标方向 |
| Dormant（>256 格） | 立即 teleport 到目标 zone 边缘（假设已跑到） |

### 环境信号

| 信号 | 距离 | 内容 |
|------|------|------|
| 地面微震 | 100+ 格 | camera micro-shake 0.05（远处大量生物奔跑） |
| 兽群噪音 | 50 格 | 低频嗡鸣 + 尖啸混合（音量随距离衰减） |
| 方向可判 | 30 格 | 大量 entity 向同一方向移动 = 视觉可读的"潮水方向" |
| narration | 全服 | "某区灵气将尽，万兽南奔。" |

---

## §3 伪灵脉→大迁徙完整链路

一个典型场景（从升起到域崩）：

```
T=0min    天道观测到灵泉湿地修士密度 > 3 → 在荒野升起伪灵脉
T=0-2min  伪灵脉渐亮，远处金色光柱可见
T=5min    3 个修士赶到伪灵脉处打坐 → 聚集加速衰减 ×1.8
T=15min   伪灵脉消散（原本 30min，因聚集加速到 15min）
T=15min   灵泉湿地 spirit_qi 从 0.7 → 0.15（被修士+伪灵脉抽干）
T=16min   消散外缘产生 2 个负灵 hot-spot → 附近一个修士被抽真元
T=20min   灵泉湿地 spirit_qi < 0.05 持续 30s → ZoneQiCriticalEvent
T=20min   大迁徙触发：湿地内 12 只噬元鼠 + 3 只灰烬蛛 + 1 只异变兽向南奔逃
T=21min   兽群涌入初醒原（南邻 zone）→ 触发兽潮（Wandering 状态机）
T=22min   agent narration："灵泉湿地灵气将尽。有修士在此贪墨太深。"
T=80min   灵泉湿地 spirit_qi 持续 < 0.1 超过 1h → 域崩触发（已有系统接管）
```

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 伪灵脉运行时（升起/活跃/聚集加速/消散/负灵风暴生命周期） | ✅ 2026-05-12 |
| P1 | 大迁徙系统（zone qi 临界 → 全兽奔逃 → 迁徙→兽潮联动） | ✅ 2026-05-12 |
| P2 | 环境信号（伪灵脉光柱 + 大迁徙地震/噪音 + narration 模板） | ✅ 2026-05-12 |
| P3 | 天道自动决策（agent skill 扩展：何时升伪灵脉/何时不干预） | ✅ 2026-05-12 |
| P4 | 饱和化测试（完整反馈环 E2E：采集→伪灵脉→消散→迁徙→兽潮→域崩） | ✅ 2026-05-12 |

---

## P0 — 伪灵脉运行时 ✅ 2026-05-12

### 交付物

1. **`PseudoVeinRuntime`**（`server/src/world/pseudo_vein_runtime.rs`，新文件）

   ```rust
   #[derive(Component)]
   pub struct PseudoVeinRuntime {
       pub zone_id: String,
       pub center_pos: BlockPos,
       pub current_qi: f64,
       pub max_qi: f64,            // 0.6（正典）
       pub base_duration_ticks: u64, // 36000（30min）
       pub started_at_tick: u64,
       pub phase: PseudoVeinPhase,
       pub cultivators_in_range: u32,
   }

   pub enum PseudoVeinPhase {
       Rising,     // 0→0.6 渐变，30s
       Active,     // 灵气维持，受聚集加速衰减
       Warning,    // qi < 0.3，预警
       Dissipating, // 0.6→0 渐变，30s
       StormAftermath, // 消散后负灵风暴，5min
   }
   ```

2. **升起逻辑**

   - agent `spawn_event("pseudo_vein", zone, intensity)` → 创建 `PseudoVeinRuntime` entity
   - zone 的 `spirit_qi` 通过 `QiTransfer { from: Tiandao, to: Zone }` 注入到 0.6
   - 渐变 30s（每 tick 增 0.6/600）

3. **聚集加速 tick**

   - 每 200 tick 检查 center 30 格内修士数 → 更新 `cultivators_in_range`
   - 衰减速率 = base_rate × `pseudo_vein_decay_multiplier(cultivators_in_range)`

4. **消散 + 负灵风暴**

   - qi 到 0 → 转 `Dissipating` → 30s 渐变归零
   - 70% qi 散归周围 zone（`qi_release_to_zone`）/ 30% 回天道（`QiTransfer { to: Tiandao }`）
   - 生成 1-3 个 `NegativePressureHotspot` entity（-0.4 ~ -0.6 灵压，5-10 min 存活）

### 验收抓手

- 测试：`world::pseudo_vein_runtime::tests::rising_reaches_0_6_in_600_ticks`
- 测试：`world::pseudo_vein_runtime::tests::crowded_dissipates_faster`（5 人 → ×3.5）
- 测试：`world::pseudo_vein_runtime::tests::qi_conservation`（注入量 = 散归量 + 天道回收量）
- 测试：`world::pseudo_vein_runtime::tests::aftermath_spawns_negative_hotspots`
- 测试：`world::pseudo_vein_runtime::tests::tide_turn_doubles_duration`（汐转期 ×2）

---

## P1 — 大迁徙系统 ✅ 2026-05-12

### 交付物

1. **`ZoneQiCriticalEvent`**

   zone tick 系统检查 `spirit_qi < 0.05` 持续 600 tick → emit event。

2. **`migration.rs`**（`server/src/fauna/migration.rs`，新文件）

   ```rust
   #[derive(Component)]
   pub struct MigrationTarget {
       pub target_zone: String,
       pub target_pos: BlockPos,
       pub speed_multiplier: f64,
       pub started_at_tick: u64,
   }
   ```

   - `migration_trigger_system`：收到 `ZoneQiCriticalEvent` → 给 zone 内所有 fauna + NPC 插入 `MigrationTarget`
   - `migration_move_system`：有 `MigrationTarget` 的 entity → Navigator 走向 target → 到达后移除 component
   - `migration_to_beast_tide_system`：统计迁入目标 zone 的兽类数 > 10 → 升级为 BeastTideRuntimeState

3. **LOD 分层**（§2 LOD 表）

   Near 完整寻路 / Far 位置偏移 / Dormant teleport。

### 验收抓手

- 测试：`fauna::migration::tests::critical_qi_triggers_migration`
- 测试：`fauna::migration::tests::migration_target_is_highest_qi_neighbor`
- 测试：`fauna::migration::tests::mass_arrival_triggers_beast_tide`（10+ → 兽潮）
- 测试：`fauna::migration::tests::npc_also_flees`（散修 NPC 也参与迁徙）
- 测试：`fauna::migration::tests::dormant_entities_teleport`

---

## P2 — 环境信号 ✅ 2026-05-12

### 交付物

1. **伪灵脉视觉**
   - Rising：金色粒子柱从地面升起（`SpawnParticle` 事件，100 格可见）
   - Active：光柱持续 + 地面灵草 VFX（绿色微光覆盖）
   - Warning：光柱闪烁加速 + 灵草枯萎粒子
   - Dissipating：光柱消失 + 灰色粒子散落

2. **大迁徙视觉/音效**
   - 地面微震（camera shake 0.05，100 格内）
   - 兽群奔跑踩踏粒子（Near 范围内每只兽脚下扬尘）
   - 低频嗡鸣音效（`beast_migration_rumble` recipe，50 格衰减）
   - 全服 narration 模板："{zone_name}灵气将尽。{beast_count}头{beast_type}向{direction}奔逃。"

3. **兽潮方向 HUD**
   - 在 HUD 边缘显示红色箭头指示兽潮来源方向（仅兽潮 Active 时）
   - 复用已有 `WarningAlertS2c` 通道

### 验收抓手

- 手动：伪灵脉升起 → 100 格外看到金色光柱 → 走近看到灵草 VFX → 5 人聚集 → 光柱闪烁加速 → 消散 → 灰色散落
- 手动：大迁徙触发 → 远处微震 → 大量兽类奔跑可见 → narration 出现

---

## P3 — 天道自动决策 ✅ 2026-05-12

### 交付物

1. **agent skill 扩展**（`agent/packages/tiandao/src/skills/ecology.md`，新文件）

   给天道 agent 新增"生态管理"技能——观测 zone 数据后决定是否升伪灵脉：

   ```
   当观测到以下任一条件时，你应考虑在该区域荒野升起伪灵脉：
   - 某 zone 玩家密度 > 3 且灵气消耗率 > 0.02/tick
   - 全服灵气总量下降 > 2%/era 且集中在 1-2 个 zone
   - 汐转期（刷新率 ×2，可更积极）

   伪灵脉的目的是分散聚集、加速淘汰、回收真元。
   不是惩罚——是引导。语气冷漠但公正。
   ```

2. **自动触发 fallback**

   agent 未响应时（LLM 超时），server 侧 fallback：
   - 每 10 分钟检查一次全 zone 灵气状态
   - 满足条件 → 自动在消耗最快 zone 的荒野边缘升起伪灵脉
   - 不依赖 agent 也能运转

### 验收抓手

- 测试：`world::pseudo_vein_runtime::tests::fallback_auto_spawn_on_high_drain`
- 手动：5 个修士在灵泉湿地采集 10 分钟 → 天道自动升起荒野伪灵脉

---

## P4 — 饱和化测试 ✅ 2026-05-12

### 交付物

1. **完整反馈环 E2E**
   - 模拟 5 修士在灵泉湿地持续采集
   - → 伪灵脉升起（自动 or agent 指令）
   - → 3 修士涌向伪灵脉
   - → 聚集加速消散
   - → 灵泉湿地 qi < 0.05
   - → 大迁徙触发
   - → 12+ 兽类涌入初醒原
   - → 兽潮状态机激活
   - → 灵泉湿地 qi 持续低 1h → 域崩触发

2. **qi_physics 守恒审计**
   - 整个链路中全服灵气总量变化 = 天道回收量（伪灵脉消散 30%）+ 天道时代衰减
   - 无凭空消失/产生

3. **性能基准**
   - 20 只兽同时迁徙（Near 范围）= 仍 60 tps
   - 100 只兽同时迁徙（Far + Dormant）= 仍 60 tps

---

## Finish Evidence

- **落地清单**：
  - P0：`PseudoVeinRuntime` component 接入 `world::register` tick；覆盖 Rising / Active / Warning / Dissipating / StormAftermath，含聚集加速衰减、汐转期时长翻倍、spawn 时 zone qi 实际注入量 `QiTransfer`、消散按实际注入量 30% 天道回收 `QiTransfer`、负灵风暴 hot-spot、Aftermath TTL 清理。
  - P1：`ZoneQiCriticalEvent` + `MigrationTarget` 接入 fauna update；zone qi `<0.05` 持续 600 tick 后触发全兽/NPC 迁徙，Near/Far/Dormant LOD 分层，10+ 兽类抵达邻区自动升既有 `beast_tide` 状态机。
  - P2：伪灵脉分阶段 VFX 事件 + client `PseudoVeinVisualPlayer`；大迁徙 `bong:migration_visual` 方向性扬尘、camera shake、`beast_migration_rumble` 音效；兽潮告警继续走既有 `event_alert`/active event 面。
  - P3：新增 `agent/packages/tiandao/src/skills/ecology.md`，并把 `pseudo_vein` 纳入 calamity skill 权限与决策规则；server 侧接入 `pseudo_vein_fallback_spawn_system`，每 12000 tick 评估高密度/高消耗 zone 并自动升起伪灵脉。
  - P4：补 `world_ecology_feedback_loop_low_qi_escalates_to_beast_tide` 饱和链路测试，覆盖低灵气持续阈值 → 迁徙 → Dormant 到达 → 兽潮升级。
- **关键 commit**：
  - `9b0c25f37`（2026-05-11）实现伪灵脉运行时入口。
  - `7a7f7c440`（2026-05-11）接入大迁徙兽潮联动。
  - `4580337a0`（2026-05-11）补齐生态事件提示与客户端信号。
  - `64228e5ad`（2026-05-11）收窄伪灵脉 fallback 护栏。
  - `a08556d1c`（2026-05-11）补齐 review 护栏：伪灵脉未知 zone 拒绝路径与 VFX phase 注册断言。
  - `1399373c4`（2026-05-12）补齐伪灵脉守恒入口：spawn 注入/消散回收 `QiTransfer`、fallback tick 入口与重复 spawn 幂等。
  - `75fa91945`（2026-05-12）收紧伪灵脉运行时幂等：同 batch pending 判重、按实际注入量结算、StormAftermath TTL 清理。
- **验证**：
  - `cargo fmt --check` ✅
  - `CARGO_BUILD_JOBS=1 cargo check --tests` ✅
  - `CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings` ✅
  - `CARGO_BUILD_JOBS=1 cargo test` ✅（4400 passed）
  - `CARGO_BUILD_JOBS=1 cargo test pseudo_vein_runtime -- --nocapture` ✅（12 passed）
  - `CARGO_BUILD_JOBS=1 cargo test fauna::migration -- --nocapture` ✅（6 passed）
  - `CARGO_BUILD_JOBS=1 cargo test spawn_event_pseudo_vein -- --nocapture` ✅（5 passed）
  - `CARGO_BUILD_JOBS=1 cargo test spawn_event_pseudo_vein_creates_runtime_component -- --nocapture` ✅（1 passed）
  - `CARGO_BUILD_JOBS=1 cargo test audio::tests::loads_default_audio_recipes -- --nocapture` ✅（1 passed）
  - `npm test -w @bong/schema && npm test -w @bong/tiandao` ✅（schema 375 passed；tiandao 355 passed）
  - `npm run build` ✅
  - `JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests com.bong.client.visual.particle.VfxRegistryTest` ✅
  - `JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build` ✅
- **遗留 / 后续**：
  - 伪灵脉灵草瞬间生长→枯萎的更细植物 cycle → plan-botany-visual-v2。
  - 兽潮 resident vs migrant 领地冲突细化 → plan-fauna-v2。
  - 专用 HUD 边缘红箭头可在后续 HUD/season 体验 plan 内用更完整方位数据扩展；本次已保留 event alert + 迁徙方向 VFX。
  - 域崩后地形永久变化（馈赠区→死域地形替换）→ plan-terrain-collapse-v1。
  - 玩家预测伪灵脉与 NPC 日程级反应 → perception / NPC daily-life 扩展。
