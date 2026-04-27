# Bong · plan-lingtian-weather-v1 · 骨架

**天气 / 季节 → 灵田生长**。把"天气季节"作为新的 `PlotEnvironment` 修饰维度，影响 plot_qi_cap 与生长曲线，长线影响补灵节奏。**严守末法世界观**：不引入五行季（火季 / 水季）、不引入"春天百花齐放"的丰收 buff——末法的天气只制造扰动与磨损，不制造馈赠。

**世界观锚点**：
- `worldview.md §十` 灵气零和——天气影响 plot 与 zone 之间的灵气流动比例，**不**新增灵气总量
- `worldview.md §六` 真元只有染色谱——**禁止**"夏季火属作物加成"
- `worldview.md §十二` 末法噬蚀——天气加剧噬蚀（雨水冲刷灵气 / 旱季蒸散）
- `worldview.md §七` 灵物密度阈值——极端天气（雷雨 / 严寒）可临时修改密度阈值（天道注视减弱 / 加重）

**library 锚点**：待写 `ecology-XXXX 末法天候录`（不基于"春夏秋冬"四季，而是末法本身的"灵气波动周期 + 偶发气象事件"）

**交叉引用**：
- `plan-lingtian-v1.md`（PlotEnvironment 已有 water_adjacent / biome / zhenfa_jvling 三槽，本 plan 加 weather/season 第 4-5 槽）
- `plan-worldgen-v3.1.md`（天气 system 来源 / biome 关联）
- `plan-narrative-v1.md`（极端气象事件可作为天道 narration 触发点）
- `plan-tribulation-v1.md`（雷雨季对渡劫的影响）

---

## §0 设计轴心

- [ ] **不做** "春夏秋冬" —— 末法世界没有四季常态，只有"灵气波动周期"（24 in-game 日 = 1 周期）
- [ ] **不做** "春耕秋收" 仪式 —— 玩家随时可种，天气只影响效率/品质而非"开放窗口"
- [ ] 天气 = **短时事件**（数小时 in-game），季节 = **长周期偏向**（数日 in-game）
- [ ] 共 4 类天气事件 + 3 档季节相位
- [ ] 季节相位：**充盈期 / 平稳期 / 枯涸期**（与 worldview §十二 末法节律契合，**不**绑定地球四季名称）

---

## §1 第一性原理（烬灰子四论挂点）

- **噬论·天候放大噬蚀**：雨季冲刷地表灵气（plot_qi_cap 临时下降）；旱季加速蒸散（plot_qi 流失加快）
- **音论·天候之音**：每种天气有自己的"音"，影响 zone qi 在 plot 之间的流动方向（雷雨 = 音乱 = plot ↔ zone 流速 +50%）
- **缚论·季节相位**：充盈期天地缚力放松 → plot_qi_cap +0.3；枯涸期缚力收紧 → plot_qi_cap -0.3
- **影论·气象事件不留镜印**：天气过去就过去，不在地块上留任何持久 buff（区别于阵法的镜印）

---

## §2 季节相位（Phase）

| 相位 | 周期占比 | plot_qi_cap 修饰 | natural_supply 修饰 | 备注 |
|---|---|---|---|---|
| **充盈期** | 6 in-game 日 | +0.3 | +20% | 玩家"农忙窗口"，但补灵冷却照常 |
| **平稳期** | 12 in-game 日 | 0 | 0 | 默认基线 |
| **枯涸期** | 6 in-game 日 | -0.3 | -20% | 鼓励玩家屯货 / 加工保鲜 |

24 in-game 日 = 1 完整周期，每个 zone 相位独立（避免全图同步无聊）。

---

## §3 天气事件（短时，数小时 in-game）

| 事件 | 持续 | plot 影响 | 触发概率 |
|---|---|---|---|
| **雷雨** | 2-4h in-game | plot_qi 与 zone qi 流速 ×1.5；plot_qi_cap 临时 -0.2 | 5% / day |
| **旱风** | 6-12h | plot_qi 衰减速率 ×2；natural_supply 临时归零 | 3% / day |
| **灵雾** | 1-2h | plot_qi_cap 临时 +0.2；natural_supply +50% | 2% / day（充盈期 ×2） |
| **阴霾** | 24h（罕见） | growth tick 暂停；天道注视密度阈值降 1 档（worldview §七 联动） | 0.3% / day |

天气事件用 server-side RNG 生成，schema 推送给 client 做粒子/天空效果。

---

## §4 数据契约

- [ ] `server/src/lingtian/environment.rs` 扩展：
  ```rust
  pub struct PlotEnvironment {
      pub water_adjacent: bool,
      pub biome: BiomeKind,
      pub zhenfa_jvling: bool,
      pub season_phase: SeasonPhase,    // 新增
      pub active_weather: Option<WeatherEvent>,  // 新增
  }
  pub enum SeasonPhase { Plenty, Steady, Drained }
  pub enum WeatherEvent { Thunderstorm, DroughtWind, LingMist, Haze }
  ```
- [ ] `server/src/lingtian/season.rs`（新文件）—— `ZoneSeasonState` Resource per zone + `season_phase_tick` system
- [ ] `server/src/lingtian/weather.rs`（新文件）—— `WeatherEventGenerator` system + `weather_apply_to_plot` 系统
- [ ] schema —— `WeatherEventDataV1` payload，含 zone / event_kind / remaining_duration
- [ ] client `weather/WeatherRenderer.java` —— 雷雨粒子 / 旱风沙尘 / 灵雾雾效 / 阴霾天空压暗
- [ ] HUD 增量：当前 zone 季节相位 mini-tag（plenty/steady/drained 三色）+ 天气事件图标

---

## §5 与 §5.1 密度阈值的耦合

- [ ] **阴霾** 事件期间，`compute_zone_pressure` 阈值临时降 1 档（worldview §七 注视减弱）—— 玩家可在阴霾窗口冒险种密集田
- [ ] **雷雨** 期间，渡劫 NPC / 玩家成功率影响（接 plan-tribulation-v1）—— 不在本 plan 实现，留 hook
- [ ] **充盈期** 不降低密度阈值，天道注视照常 —— 否则会变成"等充盈期种田 = 安全 + 高产"meta

---

## §6 实施节点

- [ ] **P0**：`SeasonPhase` enum + `ZoneSeasonState` + season_phase_tick + 接入 `PlotEnvironment` + 单测
- [ ] **P1**：plot_qi_cap / natural_supply 修饰生效 + e2e 测三相位生长差异
- [ ] **P2**：4 类 WeatherEvent + RNG 生成器 + plot 影响逻辑
- [ ] **P3**：schema + client 渲染（粒子 / 天空 / HUD tag）
- [ ] **P4**：阴霾 ↔ 密度阈值耦合 + 与 plan-narrative 接入（极端事件触发天道 narration）

---

## §7 开放问题

- [ ] 季节相位是否全 zone 同步 vs 各 zone 独立？同步会让全图玩家行为同步（可能有趣可能无聊）
- [ ] 天气事件是否可被 plan-zhenfa 阵法干预（如"挡雨阵"）？
- [ ] 阴霾的密度阈值降档是否会被滥用（玩家专挑阴霾种密集田）？需要冷却或反制
- [ ] 极端天气（雷雨 / 阴霾）是否对 NPC 散修（plan-lingtian-npc-v1）的种田 brain 产生影响？
- [ ] 客户端如何感知 zone 边界以渲染相位差异？需要 worldgen 暴露 zone 边界 API

---

## §8 进度日志

- 2026-04-27：骨架创建。前置 `plan-lingtian-v1` ✅；`plan-worldgen-v3.1` 部分 ✅。**关键风险**：worldview.md 没有现成"季节"设定，本 plan 自创"灵气波动周期 + 末法节律"概念，启动前需先在 worldview.md 补一节，否则所有 §0-§3 数值无锚点。
