# Bong · plan-jiezeq-v1 · 骨架

**末法节律系统总线**。把 worldview §十七「夏冬二季 + 汐转」从设计文档实装为 server 全局基础设施 + 跨系统 hook + agent prompt 输入 + client 间接表现。**全服同步**节律 game-tick 驱动，**完全不显式**呈现（玩家通过观察 qi 变化 / 运气波动 / 天空粒子自悟），与 §K 红线第 11 条 + O.10 决策严守对齐。

**世界观锚点**：
- `worldview.md §十七` 末法节律：夏冬二季（**本 plan 全部物理根基**——夏散冬聚 / 汐转紊乱 / 数十天压缩 / 全服天道呼吸）
- `worldview.md §二` 灵压地图："时间相位"维度（同坐标 qi_density 在二季差 20-30%）
- `worldview.md §三` 修炼时长基线（夏雷可期 / 冬力不足 / 汐转高风险）
- `worldview.md §七` 灵物密度阈值（季节修饰）
- `worldview.md §八` 天道劫气标记（汐转期触发率 ×2）
- `worldview.md §十` 资源稀缺（shelflife 衰减系数与季节耦合）
- `worldview.md §十二` 寿元（极端汐转期损耗加速）
- `worldview.md §K` 红线第 11 条 + O.10 决策（**完全不显式**——无 HUD icon / 无 narration 提示，玩家自悟）

**library 锚点**：待写 `world-XXXX 末法吐纳考`（以散修视角记录"两年观一片地"如何识破节律暗语，anchor §十七）

**交叉引用**：
- `plan-lingtian-weather-v1`（active ⏳ ~0%）—— **范围调整 / 接管**：本 plan 接管 Season enum / ZoneSeasonState / season_tick / 32 game-day 周期 / zone 同步基础设施；lingtian-weather-v1 P0 改写为"消费 jiezeq-v1 SeasonState API"，P3 HUD mini-tag 撤销（违反 §K 红线），保留 4 类 WeatherEvent + plot 影响逻辑
- `plan-botany-v2`（✅ finished）—— `HarvestHazard::SeasonRequired` stub 在 `xue_po_lian` / `jing_xin_zao` 上等本 plan 提供 `query_season` API
- `plan-tribulation-v1`（active ⏳）—— 渡劫窗口受 SeasonState 影响（夏雷可期 / 冬力不足 / 汐转高风险），本 plan **留 hook 不实装**，由 tribulation 后续接入
- `plan-shelflife-v1`（✅ finished）—— DecayProfile 系数与 SeasonState 耦合（夏 ×1.3 / 冬 ×0.7 / 汐转 ±0.2），P1 hook
- `plan-narrative-v1`（✅ finished）—— mutation skill prompt 加 season_state 输入（agent 自然带季节物象，不显式提季节名），P4 接入
- `plan-server-cmd-system-v1`（✅ finished）—— `/season` op-only slash command（query / set / advance），P0 即用
- `plan-terrain-jiuzong-ruin-v1` / `plan-terrain-pseudo-vein-v1`（✅ finished）—— 5 类地形响应规则（汐转期阵核激活 ×2 / 伪灵脉刷新 ×2），本 plan **仅留 query_season() 公共 API**，地形响应由各 terrain plan vN+1 后续接入
- `plan-cultivation-v1`（✅ finished）—— 突破成功率乘子 hook（夏 +X / 冬 -X / 汐转 ±Y），P1 接入 `breakthrough::base_success_rate`
- `plan-world-karma`（既有 `world::karma::QiDensityHeatmap` + 劫气模型）—— 汐转期劫气倍率 ×2（worldview §八），P1 接入

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `Season` enum + `WorldSeasonState` Resource + `season_tick` system + `query_season(zone, tick) -> SeasonState` 公共 API + `/season` op-only slash command + 单测 mock clock | ⬜ |
| P1 | 高 ROI hook：突破成功率乘子 + shelflife 系数 + 劫气标记倍率 | ⬜ |
| P2 | 中 ROI hook：寿元损耗加速 + 上古阵核激活率 ×2 + 伪灵脉刷新 ×2 | ⬜ |
| P3 | 暴露 `query_season` 公共 API（配套 docs + 5 类地形 hook 文档），让 plan-terrain-* / botany / lingtian-weather 各自接入 | ⬜ |
| P4 | agent 接入：world_state.season_state 字段 + mutation skill prompt 加 season 输入 + agent narration 自然带季节物象 | ⬜ |
| P5 | client 间接表现（**完全无显式 tag**）：天空颜色温度 + 灵气条饱和度 + 远景粒子（夏热浪/冬雪粒）+ 植物模型 swap hook | ⬜ |

---

## 接入面 checklist（防孤岛）

| 维度 | 内容 |
|------|------|
| **进料** | `world::tick::GameTick`（节律 tick 推进） · `world::zone::ZoneRegistry`（zone 列表，仅用于 query API 签名一致性，全服同步实际不依赖 zone） |
| **出料** | `WorldSeasonState` Resource（全服当前 SeasonState） · `query_season(zone, tick) -> SeasonState` 公共 API（mock-friendly，不直接读 Resource） · 修饰系数（突破成功率 / shelflife / 劫气倍率 / 寿元损耗）通过下游 system 主动 query · `world_state.season_state` schema 字段（agent 输入） · `bong:season_changed` event（每相位切换触发，下游可订阅） |
| **共享 event** | 复用 `world::tick::GameTick`；新增 `SeasonChangedEvent { from: Season, to: Season, tick: u64 }`（仅本 plan 内部 + agent 推送） |
| **跨仓库契约** | **server**：`Season` enum / `WorldSeasonState` Resource / `season_tick` system / `query_season` 公共 API / `SeasonChangedEvent` Bevy event / `/season` slash command<br>**agent**：`world_state.season_state: SeasonStateV1` 新字段（TypeBox in `agent/packages/schema/src/world-state.ts`）/ `mutation.md` skill prompt 加 season 输入说明 / `bong:season_changed` Redis pub<br>**client**：P5 才有契约——天空 shader patch（夏暖/冬冷温度）/ 灵气条 saturation 调整 / 远景粒子 spawn rate / **无任何文字 tag、无 HUD icon、无 narration 显式提示** |
| **worldview 锚点** | §十七 全节 + §K 红线第 11 条 + §八 劫气 + §十 shelflife + §三 突破 + §十二 寿元 |
| **红旗自查** | ❌ 自产自消（接 cultivation / shelflife / karma / agent / botany / lingtian / terrain） · ❌ 近义重名（`Season` 是新概念无碰撞；override lingtian-weather-v1 已设计的同名 enum） · ❌ 无 worldview 锚（§十七 全节直接锚定） · ⚠️ skeleton 同主题已有 lingtian-weather-v1（**本 plan 接管基础设施 + 协调范围**，不是孤立另起） · ❌ 跨仓库缺面（server + agent 都改；client 表现 P5 才有但已规划） |

---

## §0 设计轴心

- [ ] **全服同步节律**（worldview §十七 "天道呼吸" 语义）—— 不做 zone 独立相位 offset；玩家南来北往的差异由 zone qi_density 基线提供，**不再叠时间相位**。这是对 lingtian-weather-v1 已定 zone 独立的 **override**
- [ ] **完全不显式**（§K 红线第 11 条 + O.10）—— 无 HUD icon / 无文字 tag / 无 narration 显式提示；玩家通过 ① 天空颜色温度变化 ② 灵气条饱和度 ③ 粒子 ④ 修炼速度感知 ⑤ agent 隐晦物象 自悟现在是什么季节
- [ ] **game-tick 驱动**（worldview §十七 + lingtian-weather-v1 已定）—— 离线即停，回线续播；不做 wall-clock；不持久化时间戳（所有时间从 server start tick 累积）
- [ ] **节律 tick 与 vanilla MC day-night cycle 解耦** —— vanilla 20 分钟 day-night 仍走，节律是独立长周期时钟；二者无关
- [ ] **mock-friendly API** —— 所有下游 query `query_season(zone, tick)` 公共函数（不直接读 `WorldSeasonState` Resource），方便单测注入任意 tick / 任意 season
- [ ] **dev/admin 操作走 slash command** —— `/season query` / `/season set <phase>` / `/season advance <Nh|Nd|Ntick>`，op-only（普通玩家无访问，严守不显式红线）
- [ ] **不做天气事件**（雷暴 / 旱风 / 风雪 / 灵雾） —— 那是 lingtian-weather-v1 P2 范围；本 plan 仅做节律相位
- [ ] **不做 plot 影响 / 灵田专用逻辑** —— 灵田端的节律消费由 lingtian-weather-v1 实施

---

## §1 第一性原理（worldview §十七 物理推导）

- **天道呼吸残破** —— 上古一次完整呼吸需数百年，残破后压缩到数十天；故节律是末法残土的**核心时间维度**而非装饰
- **散与聚而非四季** —— 末法只有两态（夏 = 散气 / 冬 = 聚气），过渡是节律本身的紊乱（汐转）而非"春秋"
- **同坐标双世界** —— 同一片土地在夏冬之间的 qi_density 差 20-30%，本质是两个不同世界（"修士需积累至少两个完整循环才算摸过这片地"）
- **汐转双刃** —— 危险（劫气标记 ×2 / 渡劫倍险）+ 信息（同坐标在汐转前后的灵压差是地图上少有的可量化天意线索 → 老玩家"农时"的物理基础）

---

## §2 P0 — 基础设施

### 类型定义（`server/src/world/season/mod.rs` 新模块）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Summer,            // 炎汐 / 夏 40%
    SummerToWinter,    // 夏→冬 汐转 10%
    Winter,            // 凝汐 / 冬 40%
    WinterToSummer,    // 冬→夏 汐转 10%
}

impl Season {
    pub const fn is_xizhuan(self) -> bool {
        matches!(self, Season::SummerToWinter | Season::WinterToSummer)
    }
}

/// 节律周期常量。1 game-year = 48 实时小时 @ 20 tps = 3,456,000 ticks
pub const YEAR_TICKS: u64 = 48 * 3600 * 20;            // 3_456_000
pub const SUMMER_TICKS: u64 = YEAR_TICKS * 40 / 100;   // 1_382_400 (19.2h)
pub const XIZHUAN_TICKS: u64 = YEAR_TICKS * 10 / 100;  //   345_600 (4.8h)
pub const WINTER_TICKS: u64 = YEAR_TICKS * 40 / 100;   // 1_382_400 (19.2h)
// 合计 1_382_400 + 345_600 + 1_382_400 + 345_600 = 3_456_000 ✅

#[derive(Debug, Clone, Copy)]
pub struct SeasonState {
    pub season: Season,
    pub tick_into_phase: u64,    // 当前相位已走 ticks
    pub phase_total_ticks: u64,  // 当前相位总 ticks
    pub year_index: u64,         // 自 server start 第几年
}

#[derive(Resource, Default)]
pub struct WorldSeasonState {
    pub current: SeasonState,
    pub last_phase_change_tick: u64,
}
```

### 公共 API

- [ ] `query_season(zone: &str, tick: u64) -> SeasonState`（**所有下游通过此函数 query**，不直接读 Resource，方便 mock）。zone 参数当前忽略（全服同步），保留签名为未来"zone 独立"留扩展空间
- [ ] `season_tick(world_clock, &mut state, &mut event_writer)` system —— 每 tick 推进 `WorldSeasonState`；当 `tick_into_phase >= phase_total_ticks` 时切相位 + emit `SeasonChangedEvent`
- [ ] `SeasonChangedEvent { from: Season, to: Season, tick: u64 }` Bevy event

### `/season` slash command（**op-only**，含 server-cmd-system-v1 接入）

- [ ] `/season query` —— 显示当前 SeasonState（含 tick_into_phase / 距下次切相位 ticks / year_index）
- [ ] `/season set <summer|winter|xizhuan_to_winter|xizhuan_to_summer>` —— 强制切相位（清零 tick_into_phase + emit SeasonChangedEvent）
- [ ] `/season advance <N>[h|d|y|t]` —— 推进 N 实时小时 / N game-day（按 vanilla 20 min/day 算 = 24000 ticks/day）/ N game-year / N tick；**注意** game-day 这里是 vanilla 概念便于人类输入，不是节律单位
- [ ] **op-only**：检查 `Permission::Op`，普通玩家无访问；客户端不显示该命令补全
- [ ] **不向 broadcast 发任何提示** —— 即便 set 强切相位也不告诉玩家，严守不显式

### 测试（饱和化覆盖）

- [ ] **Happy path**：
  - `season_tick_advances_phase_correctly_summer_to_xizhuan`
  - `season_tick_completes_full_year_back_to_summer`
  - `season_changed_event_emits_on_phase_boundary`
- [ ] **边界**：
  - `season_state_at_tick_zero_is_summer`（约定 server start = Summer 0）
  - `season_state_at_year_ticks_minus_one_is_winter_to_summer`
  - `season_state_at_year_ticks_wraps_to_summer_year_index_plus_1`
- [ ] **错误分支**：
  - `query_season_returns_consistent_result_for_same_tick`（idempotent）
  - `query_season_with_unknown_zone_still_returns_global_state`（全服同步语义）
- [ ] **状态转换**：4 相位都有 → 4 切换 case + year_index 递增
- [ ] **slash command**：
  - `slash_season_query_returns_current_state`
  - `slash_season_set_winter_advances_to_winter_phase_zero`
  - `slash_season_advance_5h_advances_correctly`
  - `slash_season_command_rejected_for_non_op_player`
  - `slash_season_set_emits_season_changed_event`

---

## §3 P1 — 高 ROI hook（突破 / shelflife / 劫气）

### 突破成功率乘子（worldview §三 / §十七）

- [ ] `cultivation::breakthrough` 引入 `season_success_modifier(season: Season) -> f32`：
  - Summer: +5%（"夏雷可期"——尤其渡劫易得清晰雷信号）
  - Winter: -5%（"冬力不足"——稳定但爆发不足）
  - SummerToWinter / WinterToSummer 汐转: -15%（"高风险"）
- [ ] hook 点：`base_success_rate *= season_success_modifier(query_season(zone, tick).season)`
- [ ] 测试：`breakthrough_in_xizhuan_phase_has_lower_success_rate` × 4 相位

### shelflife 系数（worldview §十）

- [ ] `shelflife::types::DecayProfile` 在 lazy compute 时引入 `season_decay_modifier(season) -> f32`：
  - Summer: 衰减速率 ×1.3（"夏散" 物资蒸散加速）
  - Winter: 衰减速率 ×0.7（"冬聚" 凝固保鲜）
  - 汐转: ±0.2 RNG（紊乱）
- [ ] hook 点：`compute_freshness` 内调 `query_season` 查当前 season 应用系数
- [ ] **冻结容器例外**：在 freeze 容器内的物品忽略 season modifier（已 frozen 不受外界影响）
- [ ] 测试：`bone_coin_decays_faster_in_summer_than_winter` + 4 相位 × 2 类物品

### 劫气标记倍率（worldview §八）

- [ ] `world::karma::targeted_calamity_roll` 引入 `season_calamity_multiplier(season) -> f32`：
  - Summer / Winter: 1.0（基线）
  - 汐转: 2.0（worldview §八 "汐转期翻倍"）
- [ ] hook 点：`base_probability *= season_calamity_multiplier(...)`，clamp 到 `TARGETED_CALAMITY_MAX_PROBABILITY`
- [ ] 测试：`targeted_calamity_in_xizhuan_doubles_probability` + boundary

---

## §4 P2 — 中 ROI hook（寿元 / 阵核 / 伪灵脉）

### 寿元损耗加速（worldview §十二）

- [ ] `cultivation::lifespan` 在 `tick_age` 引入 `season_aging_modifier(season) -> f32`：
  - Summer / Winter: 1.0
  - 汐转: 1.2（极端节律下衰老加速）
- [ ] 测试：`xizhuan_phase_accelerates_aging_by_20_percent`

### 上古阵核激活率 ×2（worldview §十七 5 类地形响应表）

- [ ] **本 plan 仅暴露 query API**，实际激活率系数由 `plan-terrain-jiuzong-ruin-v1` vN+1 接入（plan-jiuzong-ruin 已 finished，需要它后续 vN+1 加 hook）
- [ ] `query_season(zone, tick).season.is_xizhuan()` 是它要查的 boolean

### 伪灵脉刷新率 ×2（worldview §十七 5 类地形响应表）

- [ ] 同上，由 `plan-terrain-pseudo-vein-v1` vN+1 接入
- [ ] 本 plan 仅 query API

---

## §5 P3 — query API 配套文档 + 5 类地形 hook 协议

- [ ] **公共 API 文档** `server/src/world/season/README.md`（plan 内文档，归档时随 finished_plans 走）：
  - `query_season(zone, tick) -> SeasonState` 调用约定
  - mock 注入示例
  - 5 类地形响应规则（worldview §十七 表）—— 列出每类地形应该如何接 hook（**本 plan 不实装**，由各 terrain plan 自接）
- [ ] **下游接入清单**（在 plan §8 数据契约表里，让审计 grep 即可定位）

---

## §6 P4 — agent 接入

### Server → agent

- [ ] `world_state.season_state` 新字段（TypeBox 定义在 `agent/packages/schema/src/world-state.ts`）：
  ```ts
  SeasonStateV1 = Type.Object({
    season: Type.Union([Type.Literal("summer"), Type.Literal("winter"),
                        Type.Literal("summer_to_winter"), Type.Literal("winter_to_summer")]),
    tick_into_phase: Type.Number(),
    phase_total_ticks: Type.Number(),
    year_index: Type.Number(),
  })
  ```
- [ ] server 端 `bong_world_state` 序列化时填入当前 SeasonState（每 zone 都填同一份，全服同步）
- [ ] `bong:season_changed` Redis pub —— 每次 SeasonChangedEvent 推送给 agent

### Agent prompt 接入

- [ ] **修改 `agent/packages/tiandao/src/skills/mutation.md`**：在 §核心法则 加一行 "你可以观察 `world_state.season_state`——它是天道呼吸的当前相位。**不要直接说现在是 X 季节**——通过物象（风信 / 云气 / 草木枯荣 / 雪线 / 暑气）暗示即可"
- [ ] 不强制 narration 必须提季节；agent 自发挥
- [ ] 测试：`mutation_narration_with_summer_season_avoids_explicit_season_name`（agent prompt 检查 narration 不含"夏 / 冬 / 汐转 / 季节"等显式词）

---

## §7 P5 — client 间接表现（**完全无显式 tag**）

### 表现清单（按 §K 红线优先级）

- [ ] **天空颜色温度**：通过 vanilla `setBlock minecraft:weather` 间接控制 + client mod patch sky color shader
  - Summer: 略偏暖（黄/橙调）
  - Winter: 略偏冷（蓝/灰调）
  - 汐转: RNG 抖动（紊乱感）
- [ ] **灵气条饱和度**（`MiniBodyHudPlanner` 既有真元条）：
  - Summer: 颜色饱和度 -10%（视觉上 "qi 稀薄"）
  - Winter: 饱和度 +10%（"qi 凝粘"）
  - 汐转: 短暂闪烁
- [ ] **远景粒子**（client mod hook）：
  - Summer: 偶发热浪折射粒子
  - Winter: 偶发雪粒
  - 汐转: 无特征粒子（紊乱 = 无规律 = 反而是"信号"）
- [ ] **植物模型 swap hook**：本 plan 仅暴露 SeasonState 给 client，植物模型 swap 由 botany-v2 vN+1 / worldgen vN+1 自接（耐热/耐寒物种切显隐）
- [ ] **绝对禁止**：
  - HUD 显示"当前：夏 / 汐转期 X 天" 文字
  - chat/narration 显式说"现在是夏季"
  - 任何 icon / badge / 进度条提示节律
- [ ] **agent narration 也要管住**：参 §6 测试 `mutation_narration_avoids_explicit_season_name`

### 测试

- [ ] client manual smoke：4 个 SeasonState 状态下天空 / 真元条 / 粒子均有可观察差异，但**没有任何文字暴露**
- [ ] 视频录制：玩家盲测——不告诉测试者"现在是什么季节"，看测试者能否通过观察猜出（猜中即设计成功；猜不中需调强表现）

---

## §8 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|---|
| `Season` enum | `server/src/world/season/mod.rs`（新模块） |
| `SeasonState` struct | `server/src/world/season/mod.rs` |
| `WorldSeasonState` Resource | `server/src/world/season/mod.rs` |
| `YEAR_TICKS` / `SUMMER_TICKS` / `XIZHUAN_TICKS` / `WINTER_TICKS` const | `server/src/world/season/mod.rs` |
| `query_season(zone, tick) -> SeasonState` 公共 API | `server/src/world/season/mod.rs` |
| `season_tick` system | `server/src/world/season/mod.rs` |
| `SeasonChangedEvent` Bevy event | `server/src/world/season/mod.rs` |
| `/season` slash command handler | `server/src/world/season/command.rs` 或并入 server-cmd-system-v1 命令树 |
| `season_success_modifier` (cultivation hook) | `server/src/cultivation/breakthrough.rs` |
| `season_decay_modifier` (shelflife hook) | `server/src/shelflife/compute.rs` |
| `season_calamity_multiplier` (karma hook) | `server/src/world/karma.rs` |
| `season_aging_modifier` (lifespan hook) | `server/src/cultivation/lifespan.rs` |
| `SeasonStateV1` TypeBox schema | `agent/packages/schema/src/world-state.ts` |
| `bong:season_changed` Redis pub | `server/src/redis_outbox.rs` + `agent/packages/tiandao/src/redis-ipc.ts` |
| mutation skill season prompt 段 | `agent/packages/tiandao/src/skills/mutation.md` |
| client 天空温度 shader patch | `client/src/main/java/com/bong/client/render/SkyColorMixin.java`（新） |
| client 真元条饱和度调整 | `client/src/main/java/com/bong/client/hud/MiniBodyHudPlanner.java` 现成 |
| client 季节粒子 spawn | `client/src/main/java/com/bong/client/render/SeasonParticleHook.java`（新） |

---

## §9 决议（立项时已闭环 10 项）

调研锚点：worldview §十七 全节 + §K 红线第 11 条 + O.10 决策 + `plan-lingtian-weather-v1`（已设计 Season enum 但未实装）+ `botany::HarvestHazard::SeasonRequired` stub + `cultivation::tick.rs:7` 注释 + `world::karma::targeted_calamity_roll` 既有概率模型 + `cultivation::lifespan` `NaturalAging` enum + `shelflife::compute` lazy decay + `agent/packages/tiandao/src/skills/mutation.md` 现有 prompt + server-cmd-system-v1 ✅ finished。

| # | 问题 | 决议 | 落地点 |
|---|------|------|--------|
| **Q1** | jiezeq-v1 vs lingtian-weather-v1 范围划分 | ✅ **A：jiezeq-v1 接管基础设施**。lingtian-weather-v1 P0 Season impl 撤销（改 wait jiezeq-v1），保留 P2 4 类 WeatherEvent + plot 影响逻辑；P3 HUD mini-tag 撤销（违反红线） | 交叉引用段 + lingtian-weather-v1 plan 文档需同步修订 |
| **Q2** | jiezeq-v1 hook 哪些下游 | ✅ P0 基础设施 / P1 高 ROI（突破 / shelflife / 劫气）/ P2 中 ROI（寿元 / 阵核 / 伪灵脉）/ P3 query API 文档 / P4 agent / P5 client 间接表现 | §2-§7 全节 |
| **Q3** | client 表现间接化程度 | ✅ **A 完全间接**：天空颜色 + 灵气条饱和度 + 粒子 + 植物模型 swap，**无任何文字 tag / HUD icon / narration 显式提示**。lingtian-weather-v1 P3 mini-tag override 撤销 | §7 + §K 红线明示 |
| **Q4** | 节律周期数值 | ✅ **48 实时 hour = 1 game-year**（重算 YEAR_TICKS = 3,456,000）。100h 玩家 ~2 完整循环，匹配 worldview "积累至少两个完整循环才算摸过这片地" | §2 类型定义 + 周期常量 |
| **Q5** | zone 独立 vs 全服同步 | ✅ **B 全服同步**（worldview §十七 "天道呼吸"语义）。lingtian-weather-v1 已定 zone offset override 撤销。`query_season(zone, tick)` 签名保留 zone 参数为未来扩展留口 | §0 设计轴心 + §2 公共 API |
| **Q6** | agent narration 集成 | ✅ **A 加 world_state.season_state 字段** + mutation skill prompt 加 season 输入说明（不强制）+ agent 自发挥用物象暗示，禁直接提季节名 | §6 全节 |
| **Q7** | jiezeq-ui-v1 是否独立 plan | ✅ **A 取消 jiezeq-ui-v1**，client 表现并入 jiezeq-v1 P5（同 plan-rat-v1 模式）。客户端表现量小且要跟 worldgen / botany 协调 | §7 全节 + journey-v1 §G 待修订 |
| **Q8** | 测试时间压缩 | ✅ **`/season` op-only slash command**（query / set / advance）+ 单测 mock clock。集成测试用 slash command 跳相位 | §2 slash command + §2 测试列表 |
| **Q9** | 渡劫窗口 hook | ✅ tribulation-v1 后续接入；本 plan 仅暴露 `query_season` API，不实装渡劫窗口逻辑 | 交叉引用段 + 数据契约 |
| **Q10** | 公共 API mock-friendly | ✅ `query_season(zone, tick) -> SeasonState` 公共函数，所有下游通过此 query（不直接读 Resource） | §0 + §2 + §8 |

> **本 plan 无未拍开放问题**——P0 可立刻起。

---

## §10 进度日志

- **2026-05-04 立项**：骨架立项。来源：用户灵感 = 末法节律系统 server + UI 都未实装。调研：worldview §十七 全节 + plan-lingtian-weather-v1（active ~0% 已设计 Season 但未落代码）+ botany::HarvestHazard::SeasonRequired stub（xue_po_lian / jing_xin_zao 等本 plan API）+ cultivation/world/karma/shelflife/lifespan 现有 hook 点。**关键发现**：lingtian-weather-v1 已设计 Season 基础设施大半但 mini-tag 违反 §K 红线 + zone 独立设计与 worldview "天道呼吸"全服同步语义不符 → 本 plan 接管基础设施 + 拉清楚边界。10 决议（Q1-Q10）一次性闭环 + jiezeq-ui-v1 取消（合并至 P5）。

## Finish Evidence

### 落地清单

- **P0 基础设施**：`server/src/world/season/mod.rs` 落地 `Season` / `SeasonState` / `WorldSeasonState` / `query_season` / `season_tick` / `SeasonChangedEvent`；`server/src/cmd/dev/season.rs` 接入 `/season query|set|advance`；`server/src/cmd/registry_pin.rs` pin 命令树。
- **P1 高 ROI hook**：`server/src/cultivation/breakthrough.rs` 接入 `season_success_modifier`；`server/src/shelflife/compute.rs` + `consume.rs` + `probe.rs` + `sweep.rs` + `variant.rs` 接入 season-aware 衰减；`server/src/world/karma.rs` + `server/src/world/events.rs` 接入汐转劫气倍率。
- **P2 中 ROI hook**：`server/src/cultivation/lifespan.rs` 接入 `season_aging_modifier`；`server/src/worldgen/pseudo_vein.rs` / `server/src/worldgen/zong_formation.rs` 既有汐转倍率测试继续锁住伪灵脉与阵核后续接入语义。
- **P3 公共 API 文档**：`server/src/world/season/README.md` 写明 `query_season(zone, tick)` 调用约定、mock 方式、下游 hook 清单。
- **P4 agent 接入**：`agent/packages/schema/src/world-state.ts` 新增 `SeasonStateV1`；`agent/packages/schema/src/channels.ts` 新增 `bong:season_changed`；`agent/packages/tiandao/src/skills/mutation.md` 要求只能用物象暗示，不直说季节名；`agent/packages/tiandao/src/world-model.ts` / `redis-ipc.ts` 消费 season 字段与事件。
- **P5 client 间接表现**：`client/src/main/java/com/bong/client/state/SeasonState*.java` + `network/SeasonStatePayload.java` 接收状态；`client/src/main/java/com/bong/client/visual/season/SeasonVisuals.java` / `SeasonVisualBootstrap.java` 实现天空 tint、真元条饱和度、远景粒子暗示；`MiniBodyHudPlanner` 只改颜色不加文字 tag。

### 关键 commit

- `d498ebf6`（2026-05-06）`plan-jiezeq-v1：落地服务端节律总线与高价值 hooks`
- `f060e5e3`（2026-05-06）`plan-jiezeq-v1：同步天道节律契约`
- `bb9ea505`（2026-05-06）`plan-jiezeq-v1：接入客户端节律暗示表现`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：通过；`cargo test` = 2452 passed。
- `cd agent && npm run build && npm test --workspace @bong/tiandao && npm test --workspace @bong/schema`：通过；tiandao = 236 passed，schema = 272 passed。
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build --no-daemon`：通过；JUnit XML 汇总 = 830 tests，0 failures，0 errors，0 skipped。
- `git diff --check`：通过。

### 跨仓库核验

- **server**：`Season` / `WorldSeasonState` / `query_season` / `season_tick` / `SeasonChangedEvent` / `SeasonCmd` / `season_success_modifier` / `season_decay_modifier` / `season_calamity_multiplier` / `season_aging_modifier` 均有编译与单测覆盖。
- **agent**：`SeasonStateV1`、`CHANNELS.SEASON_CHANGED`、`world_state.season_state` fixture、mutation prompt 禁显式季节名均有 schema / tiandao 测试覆盖。
- **client**：`SeasonStatePayload`、`SeasonStateStore`、`SeasonVisuals`、`MiniBodyHudPlanner` season 色彩路径、无 HUD 文字 tag 行为均有 JVM 测试覆盖。

### 遗留 / 后续

- 本 plan 只暴露 terrain / botany / lingtian-weather 后续消费 API，不在本 PR 内实现灵田天气事件、plot 影响、植物模型 swap 或 terrain vN+1 响应。
- 自动测试覆盖四相位数据契约与间接视觉逻辑；真实 `runClient` 盲测视频与表现强度调参保留为人工视觉验收项。
