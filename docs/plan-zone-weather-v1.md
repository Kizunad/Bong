# Bong · plan-zone-weather-v1

**zone-scoped 天气逻辑**。复用 `plan-lingtian-weather-v1` 的 `ActiveWeather` 数据层（已完成 2026-05-08，本就 zone-scoped），扩 `weather_generator_system` 接收 zone 列表 + `ZoneWeatherProfile` 概率覆写表（让"焦土雷暴 ×5"这类 zone 偏置成为一行配置而非硬编码）；并把 `WeatherEvent` 自动映射到 `plan-zone-environment-v1` 的 `EnvironmentEffect`，让"雷暴 = 一组视觉效果"成为系统自动结果而非每 plan 自己拼。

**与 lingtian-weather 的边界**：lingtian-weather 拍板了 `WeatherEvent` 5 变体（Thunderstorm / DroughtWind / Blizzard / HeavyHaze / LingMist）+ `ActiveWeather` Resource + `bong:weather_event_update` Redis pub + season 修饰常量——这些**全部不动**。本 plan 只补：**(1) zone 列表化 generator** + **(2) zone profile 概率覆写** + **(3) WeatherEvent → EnvironmentEffect 映射 system** + **(4) 物理 hook**（lightning 真劈 / 推力 / 视野遮蔽）。

**世界观锚点**：
- `worldview.md §十七 末法节律`（季节相位**全服同步**已由 jiezeq-v1 锁定——本 plan **不**做 zone 季节独立；只做"同一季节下，不同 zone 的天气事件分布偏置"）
- `worldview.md §十三 区域详情`（每个 zone 该有"招牌天气"：血谷雷劈多 / 北荒风雪长 / 焦土漂灰永不消）
- `worldview.md §八 天道激烈手段`（zone profile force_event 是天道局部干预的物理实现）
- `worldview.md §六 真元染色谱`（雷法/暴烈色 阴雨副作用 +30% 漏失——已在 lingtian-weather P0–P4 暴露 hook，本 plan 在 zone 层面把 hook 真接通）
- `worldview.md §K 信息红线`（玩家不该看到"该 zone 雷暴概率 ×5"等数值——profile 是 worldgen 配置，不在 HUD 显式）

**library 锚点**：复用 `plan-zone-environment-v1` 的 `ecology-XXXX 末法异象录`（每 zone 招牌天气作为图鉴段）。

**交叉引用**：
- `plan-zone-environment-v1.md`（**强前置**，待立）—— P1 起依赖 `EnvironmentEffect` enum + `ZoneEnvironmentRegistry`；本 plan P1 必须等 environment-v1 P0 完成
- `plan-lingtian-weather-v1.md`（finished 2026-05-08）—— `ActiveWeather` / `WeatherEvent` / `WeatherLifecycleEvent` / `try_roll_weather_for_zone` / `is_stable_tribulation_window` 全部复用；本 plan **不**改 lingtian-weather 模块（动 generator 函数签名属于 lingtian-weather P5 polish，由本 plan PR 提议但归 lingtian 模块所属）
- `plan-jiezeq-v1.md`（finished）—— 季节全服同步源；本 plan 调 `query_season(zone, tick)` 不重造
- `plan-terrain-tribulation-scorch-v1.md`（skeleton）—— 第一个真实消费方：scorch zone 配 `ZoneWeatherProfile { thunderstorm_multiplier: 5.0, drought_wind_multiplier: 2.0 }`
- `plan-tribulation-v1.md`（finished）—— `is_stable_tribulation_window` 已是 lingtian-weather 暴露的 API；本 plan 把窗口扩到任意 zone（Summer + Thunderstorm 在 zone X 仍 true）
- `plan-cultivation-v1`（active / mvp 已 finished）—— 暴烈色查询 hook 已存在；本 plan 不新建 cultivation API
- `plan-lingtian-v1.md`（active）—— PlotEnvironment 已有 active_weather 字段；本 plan **不**动 plot 侧消费

**接入面**（防孤岛）：
- **进料**：`Zone` registry（aabb / id）；`jiezeq_v1::query_season(zone, tick)`；`ZoneWeatherProfile` 配置（worldgen JSON 注册）
- **出料**：`ActiveWeather::insert(zone, event, ...)` 写入（lingtian-weather 已暴露 pub fn）；`ZoneEnvironmentRegistry::add(zone, effect)`（plan-zone-environment）；物理 hook（lightning entity / push velocity / vision obscure）
- **共享类型**：复用 `WeatherEvent` / `Season` / `ActiveWeather` / `EnvironmentEffect`（**不**新建近义 enum）
- **跨仓库契约**：复用 lingtian-weather 现有 schema + Redis channel + bridge；新增 zone profile 配置但**不**新增 wire schema（profile 是 server-side worldgen 配置）
- **qi_physics 锚点**：lightning 真劈对玩家造成的真元紊乱由 `plan-cultivation::style_modifier` 与 `qi_physics` 既有公式处理；本 plan **不引入**新真元/灵气物理常数。**红旗自检**：grep `*_DECAY*` / `*_DRAIN*` 应为 0 命中
- **守恒律自检**：weather event 不写入 `cultivation.qi_current` / `zone.spirit_qi`——天气只是触发器，所有真元转移仍走 `qi_physics::ledger::QiTransfer`

**阶段总览**：
- P0 ⬜ `ZoneWeatherProfile` 配置 + zone 列表化 generator（兼容 lingtian-weather 单 zone MVP）+ profile 概率覆写
- P1 ⬜ `WeatherEvent → EnvironmentEffect` 映射 system（依赖 plan-zone-environment-v1 P0）；5 类 weather event 各自的视觉效果 bundle
- P2 ⬜ 物理 hook 实装（lightning_strike_at / DustDevil push velocity / HeavyHaze vision obscure）+ 暴烈色染色查询 hook 接通（lingtian-weather P3 已暴露染色查询签名）
- P3 ⬜ scorch zone profile 注入（焦土雷暴 ×5 + 旱风 ×2 + 强制 lightning_strike_per_min ≥ 1.0）+ 与 plan-tribulation-v1 渡劫窗口 zone 级 hook 联动 + 4 类 narration trigger（agent → narrative）

---

## §0 设计轴心

- [ ] **不重造 ActiveWeather**：lingtian-weather 已经把 `ActiveWeather: HashMap<String, ActiveWeatherEntry>` 做成 zone-scoped；本 plan 只扩 generator 输入（zone 列表）+ 输出（zone-by-zone roll）
- [ ] **季节仍全服同步**（worldview §十七 + jiezeq-v1 锁定）：本 plan **不**做 per-zone 季节相位；zone 之间的差异**只**来自 profile（概率乘子 + 强制事件）
- [ ] **profile 是 worldgen 配置不是 wire schema**：每 zone 在 blueprint JSON 里附带 `weather_profile: { ... }`，server 启动时加载——**不**通过 IPC 推送（profile 是世界观配置，不是动态状态）
- [ ] **WeatherEvent → EnvironmentEffect 是固定映射**：Thunderstorm 永远对应一组 `[LightningPillar, EmberDrift, FogVeil(灰)]` —— 让 client 视觉一致性；profile **不**改这个映射
- [ ] **物理 hook 集中在本 plan**：lightning_strike / push_velocity / vision_obscure 这三类在本 plan 实装，scorch 等消费方不重复造（scorch §6 P2 之前规划的"server tick 强制 spawn lightning entity"现在改走本 plan hook）
- [ ] **复用 lingtian-weather 的 lifecycle event**：`WeatherLifecycleEvent::Started/Expired` 是 environment effect 的注入触发器——event Started → Registry.add(...)；Expired → Registry.remove(...)
- [ ] **zone profile 不破坏全局期望**：sum(全 zone 雷暴概率) 不超过"如果全图同 profile 的雷暴期望" × N；profile 是**重分配**而非"加更多雷"（避免天气数量膨胀）

---

## §1 ZoneWeatherProfile 设计

```rust
// server/src/lingtian/weather_profile.rs（新文件，本 plan 范围）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ZoneWeatherProfile {
    /// 各 weather kind 的概率乘子（基线 1.0）。none / 0.0 = 该 zone 不会自然 roll 此事件
    pub thunderstorm_multiplier: Option<f32>,
    pub drought_wind_multiplier: Option<f32>,
    pub blizzard_multiplier: Option<f32>,
    pub heavy_haze_multiplier: Option<f32>,
    pub ling_mist_multiplier: Option<f32>,
    /// 强制事件：每 game-day 必触发的事件（覆盖 RNG）。例：scorch zone 雷暴永不停
    pub force_event: Option<WeatherEvent>,
    /// 物理 hook 强度（仅 force_event 类型生效）：
    pub lightning_strike_per_min_override: Option<f32>,
    pub push_velocity_strength: Option<f32>,  // DustDevil 推力
    pub vision_obscure_radius: Option<f32>,    // HeavyHaze 视野遮蔽半径
}
```

### Profile 覆盖语义

| 字段 | 默认行为 | 覆盖效果 |
|---|---|---|
| `*_multiplier` | None ⇒ 1.0 | 0.5 ⇒ 该 zone 此事件概率 ×0.5；2.0 ⇒ ×2；0.0 ⇒ 永不 roll |
| `force_event` | None | 跳过 RNG，每 day 强制设为该 event（已 active 时不刷新计时器） |
| `lightning_strike_per_min_override` | None ⇒ 1.0 / min | scorch zone 配 3.0 ⇒ 雷暴期间每分 3 次雷劈 |
| `push_velocity_strength` | None ⇒ 0.5 m/s | DustDevil 推力强度（玩家侧速度增量） |
| `vision_obscure_radius` | None ⇒ 16 块 | HeavyHaze 视野遮蔽半径（vanilla fog distance 强制） |

---

## §2 WeatherEvent → EnvironmentEffect 映射

| WeatherEvent | EnvironmentEffect bundle |
|---|---|
| **Thunderstorm** | `LightningPillar { strike_rate_per_min }` + `EmberDrift { density: 0.3 }` + `FogVeil { tint: (60, 60, 70), density: 0.4 }`（黑灰云压低）|
| **DroughtWind** | `DustDevil { radius: 8, height: 30 }` + `HeatHaze { distortion_strength: 0.4 }` + `FogVeil { tint: (180, 150, 100), density: 0.2 }`（黄沙）|
| **Blizzard** | `SnowDrift { density: 0.8, wind_dir }` + `FogVeil { tint: (200, 220, 230), density: 0.7 }`（白雾）|
| **HeavyHaze** | `FogVeil { tint: (90, 90, 95), density: 0.85 }` + `AshFall { density: 0.1 }`（极致灰雾）|
| **LingMist** | `FogVeil { tint: (180, 220, 230), density: 0.5, glow: subtle }`（灵气可见的青白雾，**不带** AshFall）|

> **映射函数**：`fn weather_to_environment_bundle(event: WeatherEvent, zone_aabb: Aabb, profile: &ZoneWeatherProfile) -> Vec<EnvironmentEffect>`——纯函数，便于单测（每 event 一条 expected）。

---

## §3 物理 hook 实装

| Hook | 触发时机 | 实装位置 |
|---|---|---|
| `lightning_strike_at(pos)` | LightningPillar effect tick + roll < strike_rate_per_min | `server/src/world/weather_physics/lightning.rs`（新） |
| `apply_dust_devil_push(player, center, radius)` | 玩家进入 DustDevil center 半径内 + tick | `server/src/world/weather_physics/wind.rs`（新）。**算法移植 Weather2 (Corosauce) `spinEntityv2`**：三分量合速度 = 角度偏移（旋向，`angle += 40° * heightAmp`）+ 径向衰减（向心拉力 `pullStrength * (distMax - dist) / distMax`）+ Y 上升（`pullStrengthY * intensity`）。Bevy system 每 5 tick 遍历 zone 内 entity，写 velocity component |
| `obscure_vision(player, radius)` | 玩家在 HeavyHaze aabb 内 → server push 缩小 ViewDistance（复用 perception ViewDistance pipeline） | `server/src/world/weather_physics/vision.rs`（新） |
| `style_modifier_for_lightning_strike(player_realm, dye)` | lightning_strike 命中前 → 查暴烈色 ×0.7 + 金属甲 ×1.5 | `server/src/cultivation/style_modifier.rs::for_zone_weather`（扩展） |

**物理 hook 与 EnvironmentEffect 的耦合**：hook system 订阅 `ZoneEnvironmentLifecycleEvent`（plan-zone-environment）；当 effect Added 时 spawn marker entity 或注册 tick callback；Removed 时清理。

---

## §4 数据契约（下游 grep 抓手）

### 4.1 Server (Rust)

```rust
// server/src/lingtian/weather_profile.rs
pub struct ZoneWeatherProfile { /* §1 */ }
pub struct ZoneWeatherProfileRegistry { by_zone: HashMap<String, ZoneWeatherProfile> }

// server/src/lingtian/weather.rs（lingtian-weather 模块的扩展，本 plan 提议 PR 改）
pub fn weather_generator_system_zone_aware(
    accumulator: Res<LingtianTickAccumulator>,
    clock: Res<LingtianClock>,
    season_state: Option<Res<WorldSeasonState>>,
    zone_registry: Res<ZoneRegistry>,
    profile_registry: Res<ZoneWeatherProfileRegistry>,
    mut active: ResMut<ActiveWeather>,
    mut rng: ResMut<WeatherRng>,
    mut lifecycle: EventWriter<WeatherLifecycleEvent>,
);

// 与单 zone MVP 共存：feature flag 或 system 二选一注册（lingtian-weather P5 polish 决定）

// server/src/world/weather_to_environment.rs（新）
pub fn weather_to_environment_bundle(
    event: WeatherEvent,
    zone_aabb: Aabb,
    profile: &ZoneWeatherProfile,
) -> Vec<EnvironmentEffect>;

pub fn weather_environment_sync_system(/* WeatherLifecycleEvent → ZoneEnvironmentRegistry */);
```

### 4.2 数据契约表

| 契约 | 位置 |
|---|---|
| `ZoneWeatherProfile` struct | `server/src/lingtian/weather_profile.rs`（新） |
| `ZoneWeatherProfileRegistry` Resource | 同上 |
| `weather_generator_system_zone_aware` | `server/src/lingtian/weather.rs`（**扩展现有**，提议 PR 改 lingtian-weather 模块） |
| `weather_to_environment_bundle` 纯函数 | `server/src/world/weather_to_environment.rs`（新） |
| `weather_environment_sync_system` | 同上 |
| `lightning_strike_at` / `apply_dust_devil_push` / `obscure_vision` | `server/src/world/weather_physics/{lightning,wind,vision}.rs`（新） |
| `style_modifier::for_zone_weather` | `server/src/cultivation/style_modifier.rs`（扩展） |
| Blueprint JSON `weather_profile` 字段 | `worldgen/blueprint/zones.json`（schema 扩展） |

### 4.3 不动的契约（**复用，不新建**）

| 契约 | 来源 |
|---|---|
| `WeatherEvent` enum 5 变体 | `plan-lingtian-weather-v1` |
| `ActiveWeather` Resource | 同上 |
| `WeatherLifecycleEvent` Bevy event | 同上 |
| `is_stable_tribulation_window` / `is_xizhuan_phase` | 同上 |
| `Season` enum + 6 modifier 常量 | `plan-jiezeq-v1` |
| `EnvironmentEffect` enum 8 变体 | `plan-zone-environment-v1` |
| `ZoneEnvironmentRegistry` | 同上 |
| `Zone` / `ZoneRegistry` (aabb / id) | `server/src/world/zone.rs` |

---

## §5 实施节点

- [ ] **P0** —— ZoneWeatherProfile 配置 + zone 列表化 generator
  - 验收：`ZoneWeatherProfile` 序列化双向 round trip；profile 概率乘子单测每 weather kind × {0.5, 1.0, 2.0, 0.0} = 20 case；`weather_generator_system_zone_aware` 在 N=3 zone × profile 不同时分别 roll；`force_event` 覆盖 RNG 单测；与 lingtian-weather 单 zone MVP **共存**（feature flag 或 system 互斥注册）单测

- [ ] **P1** —— WeatherEvent → EnvironmentEffect 映射 + sync system
  - 依赖：plan-zone-environment-v1 P0 完成
  - 验收：`weather_to_environment_bundle` 5 event × profile 变化 每条都有 expected pinned；`weather_environment_sync_system` 监听 `WeatherLifecycleEvent::Started` → `ZoneEnvironmentRegistry::add` × 对应 effect；`Expired` → `remove` 单测；e2e：scorch zone 推送 Thunderstorm → client 收到 LightningPillar+EmberDrift+FogVeil

- [ ] **P2** —— 物理 hook 实装 + 暴烈色 hook 接通
  - 验收：`lightning_strike_at(pos)` spawn vanilla LightningBolt entity 单测（mock entity registry）；`style_modifier::for_zone_weather` 对暴烈色 ×0.7 / 金属甲 ×1.5 命中率公式 e2e；`apply_dust_devil_push` 玩家速度增量符号正确（朝中心或离心，按设计）；`obscure_vision` ViewDistance 临时缩减 + 玩家离开 zone 后恢复

- [ ] **P3** —— scorch zone profile 注入 + tribulation hook 联动 + narration
  - 依赖：plan-terrain-tribulation-scorch-v1 升 active
  - 验收：scorch 3 个 zone（blood_valley_east_scorch / north_waste_east_scorch / drift_scorch_001）配 profile 后；smoke test 进入 zone 看到 LightningPillar 视觉 + 雷劈频率符合 profile；`is_stable_tribulation_window(season, weather)` 在 scorch zone Summer + Thunderstorm 仍 true；agent 收到 `WeatherLifecycleEvent` → narrative 触发"焦土雷暴正盛"narration

---

## §6 测试饱和（CLAUDE.md 饱和化测试）

### P0（≥ 14 单测）
- `profile_serde_round_trip`（含 None / Some 各字段）
- `multiplier_zero_means_never_rolls`
- `multiplier_doubles_probability_within_rng_resolution`
- `force_event_overrides_rng`
- `force_event_does_not_refresh_active_timer`
- `zone_aware_generator_rolls_each_zone_independently`
- `zone_aware_generator_per_zone_last_rolled_day_dedup`
- `single_zone_mvp_compat_when_no_profile_registered`（兼容 lingtian-weather 既有行为）
- `default_profile_equivalent_to_unmodified_baseline`
- profile 5 变体 × 4 multiplier 值 = 20 case 概率分布 e2e（统计 RNG 1000 次）

### P1（≥ 8 单测）
- `weather_to_environment_thunderstorm_bundle`（断言含 LightningPillar + EmberDrift + FogVeil）
- `weather_to_environment_drought_wind_bundle`
- `weather_to_environment_blizzard_bundle`
- `weather_to_environment_heavy_haze_bundle`
- `weather_to_environment_ling_mist_bundle`
- `sync_system_started_adds_effects_to_registry`
- `sync_system_expired_removes_effects_from_registry`
- `bundle_lightning_strike_rate_uses_profile_override`

### P2（≥ 8 单测）
- `lightning_strike_spawns_vanilla_entity`
- `lightning_strike_respects_strike_rate_per_min`
- `dust_devil_push_velocity_toward_center`（或离心，按设计 pin）
- `dust_devil_push_strength_uses_profile`
- `obscure_vision_reduces_view_distance_inside_aabb`
- `obscure_vision_restores_on_zone_exit`
- `style_modifier_lightning_strike_violet_dye_x07`
- `style_modifier_lightning_strike_iron_armor_x15`

### P3（≥ 5 集成）
- `scorch_blood_valley_e2e_environment_visible`（生成 + emit + 玩家进入）
- `scorch_force_thunderstorm_overrides_summer_drought`
- `tribulation_window_scorch_summer_still_returns_true`
- `narrative_thunderstorm_started_emits_agent_hint`
- `multi_zone_generator_no_state_bleed`（zone A 雷暴不应干扰 zone B 概率）

---

## §7 开放问题

- [ ] **profile 是否运行时可改**：worldgen 启动时加载 + 不可改 vs Agent 可临时注入（"天道暂时改写血谷天气"）？倾向 v1 **启动时不可改**，agent 注入留 v2
- [ ] **lingtian-weather P5 vs 本 plan**：`weather_generator_system_zone_aware` 改的是 lingtian-weather 模块——是 lingtian-weather P5 polish 还是本 plan 的事？倾向**本 plan 提议 PR 改**，lingtian-weather 可以以"reviewer"身份介入；模块归属仍是 lingtian-weather，避免本 plan 在他处碰核心 enum
- [ ] **物理 hook 是否拆独立 plan**：lightning entity / push velocity / vision obscure 三类机制本身可大可小；首版**不拆**，留在本 plan P2；如果未来扩到"局部地震 / 局部时间流速变化"等就拆 plan-zone-physics-v1
- [ ] **vanilla 雨下不下**：本 plan **不**触动 vanilla rain（全局）。zone 内是否"看上去在下雨"由 EnvironmentEffect FogVeil + AshFall 表现，不调用 `World::set_raining`。等 v2 plan-rain-renderer-mixin 再做
- [ ] **profile 与 jiezeq 季节耦合**：profile multiplier 是否应该按季节再变化？（焦土雷暴在 Summer 已经 ×3，如果再 ×profile 5.0 = ×15 是否过度？）建议 P0 阶段评审 expected total，必要时 cap 单 day 概率上限
- [ ] **client 是否能"预知"未来天气**：lingtian-weather P3 schema 已有 `started_at` / `expires_at`；agent / client 可基于此做"预警 narration"（"血谷将起雷暴"）—— 由 plan-narrative 决定是否实装
- [ ] **scorch §6 P2 表的 hook 重定向**：scorch 骨架 §6 P2 写的 `server/src/world/weather/scorch.rs::compute_strike_chance` 应**改为**调用本 plan 的 `style_modifier::for_zone_weather`——让命中率公式只有一个唯一实现入口

---

## §8 进度日志

- **2026-05-08**：骨架立项。前置 `plan-lingtian-weather-v1`（finished 2026-05-08）已完成 zone-scoped 数据层；`plan-zone-environment-v1`（同日骨架）提供视觉协议；本 plan 是连接二者的"业务层"。
  - 起因：`plan-terrain-tribulation-scorch-v1` §8 第一项 "plan-weather-zone-override" 占位被这两份骨架吸收
  - 升 active 触发条件：（a）plan-zone-environment-v1 P0 完成（EnvironmentEffect enum 至少 4 变体落地）；（b）lingtian-weather 模块同意接受 zone-aware generator 扩展 PR；（c）scorch 骨架升 active
- **2026-05-09**：subagent 调研 MC 龙卷风 mod 结果落到本 plan：§3 物理 hook `apply_dust_devil_push` 已注明算法直接移植 Weather2 `spinEntityv2` 三分量合速度（角度偏移 + 径向衰减 + Y 上升）。Bevy 移植量约 30 行 Rust，移到 P2 工程量降级（已有参考算法可对照实装）。Weather2 同源算法亦适用 `TornadoColumn` zone 内推力，不再单独立 hook。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：`server/src/lingtian/weather_profile.rs` + `weather_generator_system_zone_aware` 扩展
  - P1：`server/src/world/weather_to_environment.rs`（mapping + sync system）
  - P2：`server/src/world/weather_physics/{lightning,wind,vision}.rs` + `cultivation/style_modifier::for_zone_weather`
  - P3：scorch 3 zone profile JSON + tribulation window hook + narration trigger
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - server：weather_profile / generator zone-aware / weather_to_environment / weather_physics / style_modifier 扩展
  - agent：narrative hook 接 WeatherLifecycleEvent
  - client：environment effect 自动渲染（依 plan-zone-environment-v1）
- 遗留 / 后续：
  - profile 运行时可改（§7 第 1 项，v2）
  - vanilla rain renderer mixin（§7 第 4 项，v2）
  - zone-physics 拆 plan（§7 第 3 项，长期）
  - 客户端天气预警 narration（§7 第 6 项，归 plan-narrative）
