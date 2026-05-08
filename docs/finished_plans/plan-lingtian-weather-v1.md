# Bong · plan-lingtian-weather-v1

> **⚠️ 2026-05-04 范围调整**：节律基础设施（`Season` enum / `WorldSeasonState` Resource / `season_tick` system / 32 game-day 周期 / zone 同步策略 / HUD 季节 mini-tag）**已转交 `plan-jiezeq-v1`**（active，2026-05-04 立项）。本 plan 范围收窄为「消费 jiezeq-v1 的 `query_season(zone, tick)` API + 4 类 WeatherEvent + plot 影响逻辑」。
>
> 受影响章节：**P0 改写**为消费 jiezeq-v1 API + PlotEnvironment 加 season/weather 槽位（不再自定义 Season enum）；**P3 撤销 HUD mini-tag**（违反 worldview §K 红线第 11 条 + journey-v1 O.10 决策"完全不显式"）；**§0 第 6 条"每 zone 季节独立" 撤销**（jiezeq-v1 决定全服同步）；**§2 周期长度段 + zone 独立段 撤销**（jiezeq-v1 接管）。详见各章节 ⚠️ 标注。

**天气 / 季节 → 灵田生长（夏冬二季 + 汐转）**。把 worldview §十七 起草的"天道吐纳二季节律"作为新的 `PlotEnvironment` 修饰维度，影响 `plot_qi_cap` 与生长曲线，长线影响补灵节奏。**严守末法世界观**：不引入五行季（火季 / 水季）、不引入"春天百花齐放"的丰收 buff——末法的天气只制造扰动与磨损，不制造馈赠。

**世界观锚点**：
- `worldview.md §十七` 末法节律：夏冬二季（**本 plan 的物理根基**——夏散冬聚、汐转紊乱）
- `worldview.md §二` 灵压环境——同坐标 qi_density 在夏冬二季差 20-30%（季节是"时间相位"维度）
- `worldview.md §十` 灵气零和——天气影响 plot 与 zone 之间的灵气流动比例，**不**新增灵气总量
- `worldview.md §六` 真元只有染色谱——**禁止**"夏季火属作物加成"
- `worldview.md §八` 天道情绪——汐转期的 qi 异常波动易触发劫气标记
- `worldview.md §七` 灵物密度阈值——极端天气（雷暴 / 严寒）可临时修改密度阈值（天道注视减弱 / 加重）

**library 锚点**：待写 `ecology-XXXX 末法天候录`（不基于"春夏秋冬"四季，锚 §十七 二季 + 汐转 + 四类气象事件 + 与生态/修炼的物理耦合）

**交叉引用**：
- `plan-jiezeq-v1`（active 2026-05-04，**强前置**）—— 节律基础设施 + `query_season(zone, tick) -> SeasonState` 公共 API；本 plan P0 起依赖该 API，jiezeq-v1 P0 必须先 finished
- `plan-lingtian-v1.md`（active）—— `PlotEnvironment` 已有 water_adjacent / biome / zhenfa_jvling 三槽，本 plan 加 season / weather 第 4-5 槽
- `plan-lingtian-process-v1.md`（与本 plan 同期升 active）—— 二级加工的 freshness 衰减速率与季节耦合（夏快冬慢）
- `plan-botany-v2.md`（active，强依赖关系）—— `xue_po_lian` / `jing_xin_zao` 的 SeasonRequired hazard 由本 plan P0–P3 提供 driver；本 plan 落地后 botany-v2 P5 回填
- `plan-tribulation-v1.md`（active）—— 雷暴对渡劫的影响：夏季雷暴 = 唯一稳定可预期天劫窗口；本 plan 留 hook 不实装
- `plan-narrative-v1.md`（骨架）—— 极端气象事件 / 汐转期天道情绪可作为 narration 触发点
- `plan-worldgen-v3.1.md`（finished）—— 天气事件不写 raster，但 zone 边界 API 用于 client 渲染

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | ⚠️ **改写**：消费 `jiezeq_v1::query_season(zone, tick)` API（**不再自定义 `Season` enum / Resource / tick system**）+ `PlotEnvironment` 扩展 season / weather 槽位 + 单测 mock query_season 注入 | ✅ 2026-05-08 |
| P1 | `plot_qi_cap` / `natural_supply` 季节修饰生效 + e2e 测二季 + 汐转生长差异 | ✅ 2026-05-08 |
| P2 | 4 类 `WeatherEvent`（雷暴 / 旱风 / 灵雾 / 阴霾）+ RNG 生成器 + plot 影响逻辑 + 季节-事件耦合（夏多雷、冬多雪/风、汐转 RNG ×2） | ✅ 2026-05-08 |
| P3 | ⚠️ **撤销 HUD 季节相位 mini-tag**（违反 §K 红线"完全不显式"）。保留 `WeatherEventDataV1` schema + client 渲染（粒子 / 天空效果——按 jiezeq-v1 P5 间接表现规范）；天气事件本身可走粒子表现，但**不显式标注当前季节** | ✅ 2026-05-08（client 粒子 / 天空渲染待 jiezeq-v1 P5 协作落地） |
| P4 | 阴霾 ↔ 密度阈值耦合 + 与 plan-narrative 接入（汐转 / 极端事件触发天道 narration）+ 与 plan-botany-v2 P5 SeasonRequired hazard 接入 | ✅ 2026-05-08 |

---

## §0 设计轴心

- [ ] **二季而非四季**：worldview §十七 锚定——末法天道呼吸残破，只剩"散（夏）"与"聚（冬）"，不存在春秋；汐转是节律本身的紊乱，不是"春"或"秋"
- [ ] **不做** "春耕秋收" 仪式 —— 玩家随时可种，季节只影响效率/品质而非"开放窗口"
- [ ] **天气 = 短时事件**（数小时 in-game），季节 = 长周期相位（数日 in-game），汐转 = 节律紊乱过渡（约一周）
- [ ] 共 4 类天气事件 + 4 个季节变体（Summer / SummerToWinter 汐转 / Winter / WinterToSummer 汐转）
- [ ] **game-tick 驱动**：离线即停，回线续播；不做 wall-clock（避免持久化时间戳 + 多人累积时间逻辑）—— ⚠️ 由 jiezeq-v1 实施，本 plan 仅消费
- [ ] ⚠️ ~~每 zone 季节独立~~ **撤销**（2026-05-04）：jiezeq-v1 决定**全服同步**（worldview §十七"天道呼吸"语义），南来北往的差异由 zone qi_density 基线提供，不再叠时间相位
- [ ] **汐转危险性**：汐转期 RNG 翻倍 + 劫气标记翻倍 + 渡劫高风险；玩家应当**学会回避汐转**而不是被告知（**劫气倍率 / 渡劫影响在 jiezeq-v1 P1 实装**，本 plan 仅消费 SeasonState 做天气事件 RNG ×2）

---

## §1 第一性原理（烬灰子四论挂点）

- **噬论·夏散冬聚**：夏季灵气随热散至上层虚空，地表 qi_density 下沉（plot_qi_cap 临时下降）；冬季灵气随寒被天地内收，qi_density 局部回升但流动性下降
- **音论·汐转之乱音**：汐转期天道吐纳间隙，节律紊乱，灵气信号嘈杂——同一 plot 的 qi 读数在数小时内可剧烈波动，破坏修士的灵感判断
- **缚论·二季之缚力差**：夏季缚力外散（plot 与外界 zone qi 流速 ×1.3），冬季缚力内收（流速 ×0.7）；汐转期缚力反复
- **影论·气象事件不留镜印**：天气过去就过去，不在地块上留任何持久 buff（区别于阵法的镜印）；唯一例外是**汐转期老玩家的"经验"**——这不是地块属性，是玩家脑中的图

---

## §2 二季 + 汐转（Season Phase）

| 季节变体 | 周期占比（典型） | plot_qi_cap 修饰 | natural_supply 修饰 | qi 流速修饰 | 触发主要天气事件 |
|---|---|---|---|---|---|
| **Summer**（炎汐） | ~40% | -0.2 | -10% | 与 zone 流速 ×1.3 | 雷暴（高频）/ 旱风 / 闷热阴霾 |
| **SummerToWinter**（夏→冬汐转） | ~10% | 反复 ±0.3 | RNG ±20% | 1.0–1.5 RNG | 全部事件 RNG ×2 |
| **Winter**（凝汐） | ~40% | +0.2 | +10% | 与 zone 流速 ×0.7 | 风雪 / 长阴霾 / 偶发灵雾 |
| **WinterToSummer**（冬→夏汐转） | ~10% | 反复 ±0.3 | RNG ±20% | 1.0–1.5 RNG | 全部事件 RNG ×2 |

> ⚠️ **2026-05-04 撤销**：以下两段（周期长度 + zone 独立）已转交 `plan-jiezeq-v1` §2 接管。jiezeq-v1 定的实际值：1 game-year = **48 实时小时** = 3,456,000 ticks（夏 19.2h / 汐转 4.8h / 冬 19.2h / 汐转 4.8h），**全服同步**。本 plan 通过 `query_season(zone, tick) -> SeasonState` 查询当前相位，上表的 plot_qi_cap / natural_supply / qi 流速修饰仍生效（这是灵田端消费逻辑）。
>
> ~~**周期长度**：1 game-year ≈ 32 game-day（夏 13 + 汐转 3 + 冬 13 + 汐转 3）—— **game-tick 驱动**，每 game-day 推进一次 phase tick。1 game-day = 24000 ticks ≈ 20 实时分钟（vanilla MC），所以 1 game-year ≈ 10.7 实时小时（玩家一次较长在线可经历一次循环）。~~
>
> ~~**zone 独立**：每个 zone 有独立 phase offset（worldgen 阶段确定，避免全图同步），zone 之间相位差最大 ±0.5 game-year，让"南来北往"的玩家能感受到节律差异。~~

---

## §3 天气事件（短时，数小时 in-game）

| 事件 | 持续 | 主出现季节 | plot 影响 | 触发概率（基线 / 主季加成） |
|---|---|---|---|---|
| **雷暴**（thunderstorm）| 2-4h | Summer | plot_qi 与 zone qi 流速 ×1.5；plot_qi_cap 临时 -0.2；夏季雷暴是 §三 渡劫的稳定窗口（hook plan-tribulation-v1） | 1% / Summer day（夏季 ×3 = 3%）/ 汐转 ×2 |
| **旱风**（drought_wind）| 6-12h | Summer | plot_qi 衰减速率 ×2；natural_supply 临时归零；shelflife 衰减 ×2 | 2% / Summer day（×3）/ 汐转 ×2 |
| **风雪 / 长阴霾**（blizzard / heavy_haze）| 12-24h | Winter | growth tick 暂停；雪线下移；天道注视密度阈值降 1 档（worldview §七）；阴霾 24h 是罕见极端事件 | 阴霾 0.5% / Winter day / 汐转 ×2；风雪 3% / Winter day |
| **灵雾**（ling_mist）| 1-2h | Winter（偶发）+ 汐转 | plot_qi_cap 临时 +0.2；natural_supply +50%；short window（玩家"农忙"信号）| 1% / Winter day / 汐转 ×3 |

事件用 server-side RNG 生成（每 game-day 开始时 roll 一次），schema 推送给 client 做粒子/天空效果。

---

## §4 数据契约（下游 grep 抓手）

### 4.1 Server (Rust)

```rust
// server/src/lingtian/environment.rs（扩展现有）
pub struct PlotEnvironment {
    pub water_adjacent: bool,
    pub biome: BiomeKind,
    pub zhenfa_jvling: bool,
    pub season: Season,                          // 新增
    pub active_weather: Option<WeatherEvent>,    // 新增
}

// server/src/lingtian/season.rs（新文件）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Summer,
    SummerToWinter,    // 汐转
    Winter,
    WinterToSummer,    // 汐转
}

impl Season {
    pub fn is_tide(self) -> bool { /* 汐转判定 */ }
    pub fn plot_qi_cap_modifier(self) -> f32 { /* §2 表 */ }
    pub fn natural_supply_modifier(self) -> f32 { /* §2 表 */ }
    pub fn zone_flow_multiplier(self) -> f32 { /* §2 表 */ }
}

#[derive(Resource)]
pub struct ZoneSeasonState {
    /// 每 zone 独立 game-day 计数（offset 在 worldgen 阶段确定）
    pub day_counter: HashMap<ZoneId, u32>,
    pub current_season: HashMap<ZoneId, Season>,
}

pub fn season_tick_system(/* ... */) { /* 每 game-day 推进 day_counter，更新 current_season */ }

// server/src/lingtian/weather.rs（新文件）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherEvent {
    Thunderstorm,
    DroughtWind,
    Blizzard,
    HeavyHaze,
    LingMist,
}

#[derive(Resource)]
pub struct ActiveWeather {
    pub events: HashMap<ZoneId, (WeatherEvent, u32 /* remaining_ticks */)>,
}

pub fn weather_generator_system(/* ... */) { /* 每 game-day RNG roll，按 §3 表 + 季节加成 */ }
pub fn weather_apply_to_plot_system(/* ... */) { /* 把 active_weather 写入 PlotEnvironment */ }
```

### 4.2 Schema（agent ↔ server / server ↔ client）

```typescript
// agent/packages/schema/src/lingtian-weather.ts
export const SeasonV1 = Type.Union([
  Type.Literal("summer"),
  Type.Literal("summer_to_winter"),
  Type.Literal("winter"),
  Type.Literal("winter_to_summer"),
]);

export const WeatherEventKindV1 = Type.Union([
  Type.Literal("thunderstorm"),
  Type.Literal("drought_wind"),
  Type.Literal("blizzard"),
  Type.Literal("heavy_haze"),
  Type.Literal("ling_mist"),
]);

export const ZoneSeasonStateV1 = Type.Object({
  zone_id: Type.String(),
  season: SeasonV1,
  day_in_year: Type.Integer({ minimum: 0, maximum: 31 }),  // 0..31 (1 year = 32 game-day)
});

export const WeatherEventDataV1 = Type.Object({
  zone_id: Type.String(),
  kind: WeatherEventKindV1,
  remaining_ticks: Type.Integer({ minimum: 0 }),
  started_at_tick: Type.Integer(),
});
```

### 4.3 Client (Java / Fabric)

```java
// client/src/main/java/.../weather/WeatherRenderer.java
public interface WeatherRenderer {
    /** 注册四类天气的粒子 / 天空效果 */
    void renderWeather(WeatherEvent event, float intensity);

    /** 根据季节修饰天空色温 + 雪线（仅冬季 / 汐转期下移） */
    void applySeasonTint(Season season, ClientWorld world);

    /** 处理事件结束的清理 */
    void clearWeather(WeatherEvent event);
}

// HUD：左上角 mini-tag 显示当前 zone 的 Season + 当前 active weather 图标
```

### 4.4 数据契约表

| 契约 | 位置 |
|---|---|
| `Season` enum (4 变体) | `server/src/lingtian/season.rs` |
| `ZoneSeasonState` Resource | `server/src/lingtian/season.rs` |
| `season_tick_system` | `server/src/lingtian/season.rs` |
| `WeatherEvent` enum (5 变体)| `server/src/lingtian/weather.rs` |
| `ActiveWeather` Resource | `server/src/lingtian/weather.rs` |
| `weather_generator_system` / `weather_apply_to_plot_system` | `server/src/lingtian/weather.rs` |
| `PlotEnvironment.season` / `.active_weather` | `server/src/lingtian/environment.rs`（扩展） |
| `SeasonV1` / `WeatherEventKindV1` / `ZoneSeasonStateV1` / `WeatherEventDataV1` | `agent/packages/schema/src/lingtian-weather.ts` + Rust 镜像 `server/src/schema/lingtian_weather.rs` |
| `WeatherRenderer` interface | `client/src/main/java/.../weather/WeatherRenderer.java` |
| HUD 季节 mini-tag | `client/src/main/java/.../hud/SeasonTagWidget.java` |
| Redis pub: `bong:zone_season_update` / `bong:weather_event_update` | server → agent ↔ client |

---

## §5 与密度阈值的耦合

- [ ] **阴霾（HeavyHaze）** 事件期间，`compute_zone_pressure` 阈值临时降 1 档（worldview §七 注视减弱）—— 玩家可在阴霾窗口冒险种密集田，但阴霾本身已经 growth tick 暂停，是机会还是陷阱由玩家判断
- [ ] **汐转期** 不直接降密度阈值，但 RNG 翻倍间接增加阴霾命中——汐转期变成隐性的"密集种田窗口"
- [ ] **雷暴** 期间 hook plan-tribulation-v1（夏季雷暴 = 渡劫稳定窗口）——本 plan **不实现**，仅暴露 `Weather::Thunderstorm` 状态供 plan-tribulation 查询
- [ ] **充盈/枯涸期不存在**——这是三相位老设计，已被 §十七 二季模型替代，不再使用

---

## §6 测试饱和（CLAUDE.md 饱和化测试）

### P0 单测（≥ 12 条）
- `season_enum_phase_modifier_summer / winter / tides`（4 条，每变体 1 条）
- `season_tick_advances_day_counter`
- `season_tick_year_wraparound`（32 → 0）
- `season_transitions_summer_to_tide_to_winter`（完整循环）
- `zone_independent_season_offset`（两 zone 不同步）
- `plot_environment_season_field_default_summer`
- `season_tick_offline_pause`（game-tick 驱动 → 离线不推进）

### P1 e2e（≥ 4 条）
- `plot_qi_cap_with_summer_modifier_drops_0_2`
- `plot_qi_cap_with_winter_modifier_rises_0_2`
- `natural_supply_in_tide_phase_random`
- `growth_curve_full_year_cycle_diff`（夏冬两季的同一作物生长曲线对照）

### P2 单测（≥ 8 条）
- `weather_thunderstorm_only_in_summer_or_tide`
- `weather_blizzard_only_in_winter_or_tide`
- `weather_tide_doubles_rng`
- `weather_active_event_blocks_growth_tick_for_haze`
- `weather_event_remaining_ticks_decrements`
- `weather_event_expires_clears_active_weather`
- `weather_thunderstorm_qi_flow_multiplier`
- `weather_drought_wind_shelflife_multiplier`

### P3 e2e（≥ 3 条）
- `client_receives_zone_season_update_payload`
- `client_receives_weather_event_payload_thunderstorm`
- `hud_season_tag_widget_renders_per_zone`

### P4 集成（≥ 3 条）
- `tribulation_thunderstorm_window_in_summer_only`（plan-tribulation hook 联动）
- `botany_v2_xue_po_lian_grows_only_in_winter_high_altitude`
- `narrative_tide_phase_triggers_tiandao_hint`

---

## §7 实施节点（详细）

- [ ] **P0**：`Season` enum 4 变体 + `ZoneSeasonState` Resource + `season_tick_system` + `PlotEnvironment.season` 字段 + §6 P0 单测全绿（12 条）；不动 weather；不动 client
- [ ] **P1**：把 `Season::*_modifier()` 接入 `plot_qi_cap` / `natural_supply` 计算路径 + §6 P1 e2e（4 条）；e2e 用 fixture 模拟一年完整循环
- [ ] **P2**：`WeatherEvent` enum 5 变体 + `ActiveWeather` Resource + `weather_generator_system` + `weather_apply_to_plot_system` + §6 P2 单测（8 条）+ 季节-事件耦合矩阵
- [ ] **P3**：schema `SeasonV1` / `WeatherEventKindV1` / `ZoneSeasonStateV1` / `WeatherEventDataV1` 双端镜像 + Redis pub 通道 + client `WeatherRenderer` 实装（粒子 / 天空 / 雪线下移）+ HUD `SeasonTagWidget` + §6 P3 e2e（3 条）
- [ ] **P4**：阴霾 ↔ 密度阈值耦合 + plan-narrative-v1 接入（汐转 / 极端事件触发天道 narration）+ plan-botany-v2 P5 `SeasonRequired` hazard 接入（`xue_po_lian` Winter only / `jing_xin_zao` 汐转 driver）+ §6 P4 集成（3 条）

---

## §8 验收

| 阶段 | 验收条件 |
|---|---|
| P0 | 4 季节变体 + tick 系统落地；P0 12 条单测全绿；32 game-day 完整循环可重现 |
| P1 | 同 zone 同 plot 在夏 / 冬两季的 plot_qi_cap 差为 0.4（夏 -0.2 + 冬 +0.2）；汐转期 RNG 波动覆盖测试 |
| P2 | 4 类天气事件按季节加成 RNG 触发；事件期间影响生效；汐转期 RNG 翻倍可验证 |
| P3 | client 能接收 schema payload 并渲染；HUD season tag 显示当前 zone 季节；雪线在冬季 + 汐转期下移 |
| P4 | 跨 plan e2e：tribulation 雷暴窗口 / botany-v2 雪魄莲季节限定 / narrative 汐转 hint 全部命中 |

---

## §9 风险与缓解

| 风险 | 缓解 |
|---|---|
| 32 game-day = 10.7 实时小时，玩家可能错过一整年的某些事件 | 多人服务器累积时间足够；单机玩家短期内多见汐转，长期内见完整年；可在 P3 后视玩家反馈调整周期数（本 plan 默认 32，不锁死） |
| zone 独立季节会让玩家"地图熟练度"曲线陡 | 这是设计意图（worldview §十七"老玩家的核心是记得汐转曲线"）；HUD 季节 tag 提供基础信息支持，不喂饭 |
| 汐转期 RNG 翻倍可能撞上玩家高强度活动 | 玩家通过 HUD 可知当前是否汐转；老玩家会主动避开；新手在此撞死是末法残土的常态学费（worldview §十五） |
| 季节修饰与 plan-shelflife 的相互影响 | 夏季 shelflife 衰减 ×2 已在 §3 旱风事件 / §10 列出；与 plan-shelflife-v1 的 Exponential profile 兼容（额外乘数）；P4 集成时核验 |
| game-tick 驱动 vs 多人服务器：玩家短在线感受不到季节变化 | 由 ZoneSeasonState 在 server 全局推进（非 per-player），所有在线玩家共享同一 zone 的季节状态——单玩家短在线感受到的是"片段"，但服务器整体连续 |

---

## §10 开放问题（升 active 后再决议）

- [ ] **HUD 季节 mini-tag 的精确视觉**：四色 tag（Summer 红 / SummerToWinter 黄 / Winter 蓝 / WinterToSummer 紫）？还是图标 + 文字？归 plan-HUD-v1 的视觉统一处理
- [ ] **天气事件是否可被 plan-zhenfa 阵法干预**（如"挡雨阵"）？v1 不实现，留 v2 决策
- [ ] **NPC 散修是否感知季节**（plan-lingtian-npc-v1）？理论上 NPC AI 应当看天气调整种田策略；本 plan 仅暴露状态，npc-v1 自决
- [ ] **客户端如何感知 zone 边界以渲染相位差异**？需要 plan-worldgen-v3.x 暴露 zone 边界 API；P3 启动时核验
- [ ] **季节-加工耦合的 multiplier 数值**：与 plan-lingtian-process-v1 的 freshness 衰减如何叠加（夏 ×2 是绝对乘数还是相对修正）？P4 集成时与 process-v1 共同决议

---

## §11 进度日志

- **2026-04-27**：骨架创建。前置 `plan-lingtian-v1` ✅；`plan-worldgen-v3.1` 部分 ✅。**关键风险**：worldview.md 没有现成"季节"设定，本 plan 自创"灵气波动周期"概念。
- **2026-04-29**：worldview.md §十七 已落地（夏冬二季 + 汐转 + game-tick 驱动 + zone 独立），关键风险解除。本 plan 三相位（Plenty/Steady/Drained）模型废弃，重写为二季 + 汐转 4 变体。同步与 plan-lingtian-process-v1（freshness 选 game-tick）+ plan-botany-v2（P5 SeasonRequired driver 反向接入）一致。`plan-alchemy-v1` / `plan-forge-v1` 已归档（非依赖）；`plan-tribulation-v1` 是 hook 关系不阻塞。准备升 active。

---

## Finish Evidence

> 2026-05-08 全 P 落地。consume-plan workflow auto-commit。本 plan 范围已收窄
> 为"消费 jiezeq-v1 季节 API + 5 类 WeatherEvent + plot 影响 + 跨 plan hook"，
> 不重造 Season 基础设施。

### 落地清单（按阶段）

**P0 — Season modifier + WeatherEvent enum + PlotEnvironment 槽位**
- `server/src/world/season/mod.rs` —— Season 加 6 个 modifier 常量（plot_qi_cap_modifier
  / natural_supply_modifier / zone_flow_multiplier / xizhuan_qi_cap_amplitude /
  xizhuan_supply_amplitude / xizhuan_zone_flow_jitter_max_delta + weather_rng_multiplier）
  + Default impl pin Summer
- `server/src/lingtian/weather.rs`（新文件）—— WeatherEvent enum 5 变体 + 7 个
  修饰常量函数（as_wire_str / blocks_growth_tick / plot_qi_cap_delta /
  zone_flow_multiplier / qi_decay_multiplier / natural_supply_multiplier /
  shelflife_decay_multiplier / pressure_threshold_relax_steps / all）
- `server/src/lingtian/environment.rs` —— PlotEnvironment 加 season + active_weather
  槽位；compute_plot_qi_cap 接 Season + WeatherEvent 修饰；apply_xizhuan_qi_cap_jitter
  / apply_xizhuan_supply_jitter / compute_zone_flow_multiplier helper
- `server/src/lingtian/systems.rs` —— till test 适配新字段（用 SummerToWinter 锁
  modifier=0 保留 plan-lingtian-v1 的 cap=2.8 锁定）

**P1 — natural_supply / zone_pressure 接季节修饰**
- `server/src/lingtian/pressure.rs` —— effective_natural_supply（base × season_delta
  × weather_mult）+ derive_supply_jitter（per-day stable hash）+ compute_zone_pressure
  签名加 season + jitter + weather 参数
- `server/src/lingtian/systems.rs::compute_zone_pressure_system` —— 从
  `Option<Res<WorldSeasonState>>` 取季节，调用 derive_supply_jitter 算 jitter
- 测试 fixture `build_pressure_app_with_season` 显式 pin 季节（pin Summer 默认）

**P2 — WeatherEvent 完整生成器 + ActiveWeather Resource**
- `server/src/lingtian/weather.rs`：
  - `WeatherEvent::daily_probability` —— 完整 §3 表（5 变体 × 4 季节 = 20 case）
  - `WeatherEvent::can_occur_in` / `duration_range_lingtian_ticks`
  - `ActiveWeather` Resource（zone → ActiveWeatherEntry，含 last_rolled_day 去重）
  - `WeatherRng`（独立 xorshift64 + next_f32 / next_u64_range）
  - `try_roll_weather_for_zone`（纯函数 RNG roll，单测可注入）
  - `weather_generator_system` / `weather_apply_to_plot_system`
- `server/src/lingtian/mod.rs` —— 注册 ActiveWeather + WeatherRng Resource +
  weather_generator + weather_apply 在 lingtian_growth_tick 之前跑

**P3 — Schema 双端镜像 + Redis pub + Bevy lifecycle event**
- `agent/packages/schema/src/lingtian-weather.ts`（新文件）—— WeatherEventKindV1 +
  WeatherEventDataV1 + WeatherEventUpdateV1 + 验证函数
- `agent/packages/schema/src/index.ts` / `schema-registry.ts` / `channels.ts` ——
  注册 + CHANNELS.WEATHER_EVENT_UPDATE
- `agent/packages/schema/samples/weather-event-{data,update}.sample.json` —— 双端对拍
- `agent/packages/schema/generated/weather-event-{kind,data,update}-v1.json` ——
  `npm run generate` 产物
- `server/src/schema/lingtian_weather.rs`（新文件）—— Rust 镜像 +
  From<WeatherEvent> / From<WeatherEventKindV1>
- `server/src/schema/channels.rs` —— `CH_WEATHER_EVENT_UPDATE` 常量
- `server/src/lingtian/weather.rs::WeatherLifecycleEvent` —— Bevy event Started/Expired
- `server/src/network/redis_bridge.rs` —— `RedisOutbound::WeatherEventUpdate` +
  序列化 arm
- `server/src/network/weather_bridge.rs`（新文件）—— `publish_weather_lifecycle_events`
  系统：读 Bevy event 转 RedisOutbound

**P4 — 阴霾 ↔ 密度阈值耦合 + tribulation/narrative hook**
- `server/src/lingtian/pressure.rs::PressureLevel::classify_with_relax` —— 通用
  N 档放宽方法
- `server/src/lingtian/systems.rs::compute_zone_pressure_system` —— 阴霾 active
  时调用 `pressure_threshold_relax_steps(=1)` 派生最终档位
- `server/src/lingtian/weather.rs::is_stable_tribulation_window` —— Summer +
  Thunderstorm 唯一返回 true（plan-tribulation-v1 hook）
- `server/src/lingtian/weather.rs::is_xizhuan_phase` —— plan-narrative-v1
  hint 触发条件（汐转 = 天道情绪起伏）

### 关键 commit

| Hash | 日期 | 描述 |
|---|---|---|
| `81feec6fb` | 2026-05-08 | feat(lingtian-weather): P0 — Season modifier + WeatherEvent enum + PlotEnvironment 槽位 |
| `dd0cf61eb` | 2026-05-08 | feat(lingtian-weather): P1 — natural_supply / zone_pressure 接季节修饰 |
| `3db22b955` | 2026-05-08 | feat(lingtian-weather): P2 — WeatherEvent 生成器 + ActiveWeather Resource + 系统注册 |
| `8b11ac761` | 2026-05-08 | feat(lingtian-weather): P3 — schema 双端镜像 + Redis pub + Bevy lifecycle event |
| `cbf5de578` | 2026-05-08 | feat(lingtian-weather): P4 — 阴霾↔密度阈值耦合 + tribulation/narrative hook |

### 测试结果

| 命令 | 通过数 | 备注 |
|---|---|---|
| `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` | **2643 passed / 0 failed** | 全工程跑过；本 plan 新增 ~70 单测 / 集成测 |
| `cd agent && npm run build` | OK | typebox 编译通过 |
| `cd agent/packages/schema && npm test` | **292 passed / 0 failed** | 含 9 个新 weather schema tests + 4 generated-artifacts gate |

§6 Test 饱和清单（plan 要求 ≥30 条）已超额：
- §6 P0 单测 12 条 → 实装 ~37 条（13 season + 11 weather + 14 environment）
- §6 P1 e2e 4 条 → 实装 12 条（pressure effective_supply / jitter spread / cycle delta + systems integration）
- §6 P2 单测 8 条 → 实装 18+ 条（probability table / ActiveWeather / WeatherRng / try_roll
- §6 P3 e2e 3 条 → 实装 9 TS + 6 Rust schema mirror + 4 bridge = 19 条
- §6 P4 集成 3 条 → 实装 11 条（classify_with_relax 7 + system integration 2 + hook helpers 2）

### 跨仓库核验

**server (Rust)**：
- `crate::lingtian::weather::WeatherEvent` / `ActiveWeather` / `ActiveWeatherEntry` /
  `WeatherRng` / `WeatherLifecycleEvent` / `is_stable_tribulation_window` /
  `is_xizhuan_phase` / `try_roll_weather_for_zone` / `weather_generator_system` /
  `weather_apply_to_plot_system`
- `crate::lingtian::environment::PlotEnvironment.{season, active_weather}` /
  `apply_xizhuan_qi_cap_jitter` / `apply_xizhuan_supply_jitter` /
  `compute_zone_flow_multiplier`
- `crate::lingtian::pressure::effective_natural_supply` / `derive_supply_jitter` /
  `PressureLevel::classify_with_relax`
- `crate::world::season::Season::{plot_qi_cap_modifier, natural_supply_modifier,
  zone_flow_multiplier, xizhuan_qi_cap_amplitude, xizhuan_supply_amplitude,
  xizhuan_zone_flow_jitter_max_delta, weather_rng_multiplier}` + `Default for Season`
- `crate::schema::lingtian_weather::{WeatherEventKindV1, WeatherEventDataV1,
  WeatherEventUpdateKindV1, WeatherEventUpdateV1}`
- `crate::schema::channels::CH_WEATHER_EVENT_UPDATE`
- `crate::network::weather_bridge::publish_weather_lifecycle_events`
- `crate::network::redis_bridge::RedisOutbound::WeatherEventUpdate`

**agent (TypeScript)**：
- `WeatherEventKindV1` / `WeatherEventDataV1` / `WeatherEventUpdateV1` 在
  `@bong/schema` 包导出 + 注册 SCHEMA_REGISTRY
- `CHANNELS.WEATHER_EVENT_UPDATE = "bong:weather_event_update"`
- generated/weather-event-{kind,data,update}-v1.json
- samples/weather-event-{data,update}.sample.json

**client (Java/Fabric)**：
- 本 plan 不交付 Java 端实装。原计划 `WeatherRenderer.java`（粒子 / 天空效果）+
  `SeasonTagWidget.java` 中 SeasonTagWidget 已被 plan §P3 撤销；WeatherRenderer
  按 plan 引用"jiezeq-v1 P5 间接表现规范"，但 jiezeq-v1 P5（client 间接表现）
  目前仍 ⬜，故本 plan 留待 jiezeq-v1 P5 协作落地后再开 PR。

### 遗留 / 后续

- **plan-lingtian-weather-v1 自身待补**（依赖外部 plan）：
  - **client 粒子 / 天空效果**：等 plan-jiezeq-v1 P5 落地"间接表现规范"后协作开发；
    本 plan 已暴露 schema / Redis 通道 / Bevy event，client 可直接消费
  - **plot_qi_cap weather 实时修饰**：plan §3 要求雷暴 -0.2 / 灵雾 +0.2 临时修饰
    plot_qi_cap，本 plan 未在 plot 上动态 mutate（plot.plot_qi_cap 是 till 时
    snapshot）。落地需求：plot 加 base_plot_qi_cap + active_weather_delta_cache
    字段或在使用站调用 effective_plot_qi_cap_with_weather 派生。**留 P4+ polish**

- **下游 plan 接入**（hook 已暴露，等下游 PR 消费）：
  - **plan-tribulation-v1**（finished）：消费 `is_stable_tribulation_window` 决定
    渡劫窗口稳定性 / 心魔劫窗口
  - **plan-narrative-v1**（skeleton）：消费 `is_xizhuan_phase` + `WeatherLifecycleEvent`
    Bevy event 触发暗示性 narration
  - **plan-botany-v2**（active P5）：通过 `Season::is_xizhuan` /
    `WeatherEvent::can_occur_in` 直接消费，无需额外 helper（plan §6 列出的
    `xue_po_lian` Winter only / `jing_xin_zao` 汐转 driver 在 botany-v2 端实装）
  - **plan-lingtian-npc-v1**（骨架）：NPC AI 季节感知留 npc-v1 自决
  - **plan-lingtian-process-v1**（active）：shelflife 衰减乘数已暴露
    `WeatherEvent::shelflife_decay_multiplier`，process 端在 freshness 派生时调用即可

- **HUD 视觉细节**：plan-HUD-v1（未归档）协调统一处理；本 plan 已确认
  撤销 HUD season tag（worldview §K 红线"完全不显式"），不再有 widget 需求

- **NPC 季节感知**：plan-lingtian-npc-v1 自决（NPC AI 应当看天气调整种田策略，
  本 plan 仅暴露 ActiveWeather Resource 供查询）

- **多 zone 扩展**：本 plan MVP 单 zone（DEFAULT_ZONE）；多 zone 时
  `try_roll_weather_for_zone` / `WeatherLifecycleEvent` 已支持 zone 标识，
  接 worldgen-v3.x 的 zone 边界 API 后扩展即可
