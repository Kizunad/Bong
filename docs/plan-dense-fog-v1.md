# Bong · plan-dense-fog-v1

**区域浓雾（雾堤）**：把 `FogVeil.density ≥ 0.85` 正式定义为跨端统一的「浓雾档」——client 渲染出"勉强伸手不见五指"（fogEnd ≈ 8 格）的真雾，server 提供不被天气 sync 冲掉的**动态雾堤注入源**（任意 AABB + TTL），天道 agent 获得下雾的命令入口。补齐 `plan-zone-environment-v1` 归档遗留第 5 项「Agent → server 注入 API」。

**日期约定**：`YYYY-MM-DD` 均为 Pacific/Auckland 本地日期。

## 为什么不加新 enum 变体（核心设计决策）

- server `weather_physics/vision.rs:11` 已存在 `OPAQUE_FOG_DENSITY_THRESHOLD = 0.85`：density ≥ 0.85 的 FogVeil 会把该 zone 客户端 `ViewDistance` 压到 `vision_obscure_radius`（默认 16 格，`lingtian/weather_profile.rs:19`）——**服务端权威的"看不见"已经在跑**（chunk 不发，透视外挂也看不穿），天气 `HeavyHaze`（density 0.85，`weather_to_environment.rs:71-83`）已是触发者
- wire `FogVeil { aabb_min, aabb_max, tint_rgb, density }` 本来就支持任意 AABB 子区域（`server/src/world/environment.rs:32-37`），**本 plan 零 schema 漂移**（P2 天道命令除外）
- 缺口全在两头：client 渲染公式钳死在 fogEnd ≥ 44（`EnvironmentFogPlanner.java:32-33`，到不了伸手不见五指）；server 没有绕过每帧 `replace_for_dimension` 全量覆盖（`environment.rs:301`）的动态注入面
- 复用 density 语义 → HeavyHaze 天气**自动**升级成真浓雾，第一个消费方零接线

**交叉引用**：
- `plan-zone-environment-v1.md`（finished）—— FogVeil 协议 / `ZoneEnvironmentRegistry` / `EnvironmentFogPlanner` 全部由它立，本 plan 是其"浓雾档"延伸 + 遗留第 5 项收口
- `plan-zone-weather-v1.md`（finished）—— `weather_to_environment.rs` 天气→FogVeil 映射表；HeavyHaze/LingMist 数值 owner，P3 调参在本 plan PR 内改动需注明
- `plan-zone-atmosphere-v2.md`（finished）—— `mergeFogCommands` 双源合并语义（`ZoneAtmosphereRenderer.java:105-121`），浓雾档沿用 min-combine 不重造
- `plan-perception-v1.1.md`（finished）—— RealmVision fog sink（`GlFogParamsSink.java:9-12`）与环境雾 sink 同注入点冲突，P0 必须仲裁
- `plan-woliu-path-v1.md`（finished）—— `wangyintai_atmosphere.rs` 的 `build_fog_veil()` 生产无调用者（半孤儿），本 plan **不接**，仅在 §8 登记

**接入面**（防孤岛）：
- **进料**：`ZoneEnvironmentRegistry`（`server/src/world/environment.rs`）；`ActiveWeather`（`lingtian/weather.rs`，HeavyHaze 现成触发者）；`bong:agent_cmd`（P2 新命令类型）；`/fog` dev 命令（P1）
- **出料**：`bong:zone_environment` S2C（channel / 形状均不变）→ client `EnvironmentFogPlanner` 浓雾分段渲染；`weather_vision_obscure_system` ViewDistance 压缩（已存在，本 plan 只对齐阈值语义不改其逻辑）；天道 narration 天象预兆（P2）
- **共享类型**：复用 `EnvironmentEffect::FogVeil`（**不新增变体**）、`OPAQUE_FOG_DENSITY_THRESHOLD`（跨端语义 pin）、`ZoneWeatherProfile.vision_obscure_radius`；client 复用 `EnvironmentFogCommand` / `FogVeilEmitter` / `mist_low_loop`
- **跨仓库契约**：server `EnvironmentOverlays` Resource + `CommandType` 新增 `spawn_fog_bank`（`agent/packages/schema/src/common.ts:29-38` union 扩展 + samples）↔ client `EnvironmentFogPlanner` 浓雾分段 + 天空遮蔽；`bong:zone_environment` payload 不变
- **worldview 锚点**：§八「天道的手段·中等手段——发布天象预兆让修士自行迁移」（浓雾 = 天象预兆的视觉本体）；§二「负灵域·游离风暴」（移动负能风暴的视觉载体，本 plan 只做静态雾堤表现，游离物理见 §8）；§K 信息红线（雾是氛围与信息压制，无任何 HUD 数值/图标）
- **qi_physics 锚点**：**无**——纯表现层 + ViewDistance 压缩，不引入任何衰减/物理常数，不读写 `spirit_qi` / `qi_current`。红旗自检：本 plan 实装代码 grep `DECAY|DRAIN|RHO|BETA` 0 命中

**阶段总览**：
- P0 ⬜ client 浓雾渲染档：分段曲线 + 天空/日月星遮蔽 + AABB 边界羽化 + RealmVision sink 仲裁
- P1 ⏳ server 动态雾堤：`EnvironmentOverlays` Resource（AABB + TTL，并入 sync 组装）+ `/fog` dev 命令 + bot 场景（主体已由「/fog 动态雾堤」PR 交付，见 §9；余：`weather_vision_obscure_system` 集成用例 + 新客户端 join 补发含 overlay 用例）
- P2 ⬜ 天道接线：`spawn_fog_bank` CommandType + executor handler + narration 预兆 + 双端 sample
- P3 ⬜ 消费方校准：HeavyHaze/雾堤 runClient 截图基线 + 数值调参 + e2e

---

## P0 — client 浓雾渲染档

**目标**：density ≥ 0.85 时 client 真正"看不见"，且与现有 < 0.85 各 zone 视觉完全兼容（现行公式段不动，pin 测试不红）。

交付物（`client/src/main/java/com/bong/client/environment/` + `mixin/`）：

1. **分段雾距曲线**（`EnvironmentFogPlanner`）：
   - `d < 0.85`：维持现行 `fogStart = 28 − 18d`，`fogEnd = 96 − 52d`（tsy 0.58 / tribulation 0.42 / scorch 0.34 视觉不变，现有测试不动）
   - `d ∈ [0.85, 1.0]`：`t = (d − 0.85) / 0.15`；`fogStart = 12.7 − 11.2t`（→ 1.5），`fogEnd = 51.8 − 43.8t`（→ 8.0）。两段在 0.85 处连续
   - 阈值常量 `DENSE_FOG_THRESHOLD = 0.85` 与 server `OPAQUE_FOG_DENSITY_THRESHOLD` 双端 pin 测试互锁（值漂移即红）
   - 浓雾档 `FogShape` 由 `CYLINDER` 切 `SPHERE`（头顶不漏光）
2. **天空遮蔽**（`EnvironmentSkyController` + `MixinSkyPerZone`）：遮蔽因子 `o = clamp01((d − 0.85) / 0.10)`；sky shader color 向雾色混合权重从现行 0.45 封顶改为 `max(0.45·比例, o)` 直至全遮；`o ≥ 0.5` 时跳过日月星/云渲染（`renderSky` 内 celestial 段早退）。雾色 = `tint_rgb` 全权重（浓雾档不再与 BASE_SKY 混）
3. **AABB 边界羽化**（`EnvironmentFogPlanner`）：替换二值 `contains()` 门——`edgeFactor = clamp01(min(玩家到六面内侧距离) / 12.0)`，`有效 d = density × alpha × edgeFactor`。修掉现存"跨界瞬跳"（`EnvironmentFogPlanner.java:20-21` 硬 contains），对 < 0.85 的既有雾同样生效（顺带改善）
4. **fog sink 仲裁**：`EnvironmentFogController` 与 `RealmVisionFogController` 同在 `applyFog` TAIL 且 mixin 优先级相同（注入顺序未定义，实测互覆）。收敛为单一 `FogArbiter`：两命令 min-combine（fogStart/fogEnd 取更近，同 `mergeFogCommands` 语义），两个 mixin 只留一个写 GL
5. **音效/粒子**：复用 `FogVeilEmitter`（密度驱动粒子预算已存在）+ `mist_low_loop` ambient（`EnvironmentEffect.java:216` 已接）；浓雾档不新增音效协议

**测试**（Java，饱和）：分段曲线两段各 3 点 + 0.85 连续性 + d=1.0 极值 pin；羽化边界 0/6/12 格三点 + 盒外 0；天空遮蔽 o 曲线 0.84/0.90/0.95 三点 + celestial 跳过分支；仲裁 min-combine 四象限（env 浓/RV 浓/双活/双空）；现有 `EnvironmentFogPlannerTest` 全绿不改断言。

## P1 — server 动态雾堤 + dev 命令

**目标**：在任意 AABB 上起一片有寿命的浓雾，且**不被** `weather_environment_sync_system` 每帧 `replace_for_dimension`（`environment.rs:301`）**冲掉**。

交付物：

1. `server/src/world/environment_overlay.rs`：`EnvironmentOverlays` Resource——`Vec<FogBank { id: String, dimension: String, aabb_min/max: [f64;3], tint_rgb: [u8;3], density: f32, expires_at_tick: Option<u64> }>`；**merge 进 `sync_zone_environment_effects` 的组装结果**（按 AABB 中心命中 zone 附着），不走会被覆盖的 `registry.add()`；过期自动摘除并 mark dirty
2. `/fog` dev 命令（`server/src/cmd/dev/fog.rs`，仿 `zone_qi.rs` 范式）：`/fog spawn <x1> <y1> <z1> <x2> <y2> <z2> <density> [duration_ticks]`、`/fog clear [id]`、`/fog list`；登记 `cmd/registry_pin.rs` + `cmd/completions.rs`；CLAUDE.md dev 命令表补一行（dev-only 标注）
3. bot 场景 `scripts/bot/scenarios/`：`/fog spawn` 后断言收到 `bong:zone_environment` payload 含 density ≥ 0.85 的 fog_veil、TTL 到期后收到摘除后的 payload
4. 与 `weather_vision_obscure_system` 的交集验证：雾堤 density ≥ 0.85 时同样触发 ViewDistance 压缩（现有 system 读 registry effects，overlay merge 后天然命中——集成测试锁住）

**测试**（Rust，饱和）：overlay merge 进组装 / 与天气 FogVeil 共存 / TTL 到期摘除 + dirty / 跨 dimension 隔离 / `/fog` 三子命令 happy + 非法 AABB + 越权 density clamp / vision_obscure 集成命中 / 新客户端 join 补发含 overlay。

## P2 — 天道接线（补 zone-environment-v1 遗留第 5 项）

**目标**：天道 agent 能以「天象预兆」形式下雾（worldview §八 中等手段）。

交付物：

1. `agent/packages/schema/src/common.ts` `CommandType` union 新增 `"spawn_fog_bank"`；`agent-command.ts` 参数：`{ aabb_min, aabb_max, tint_rgb?, density, duration_ticks, narration? }`；samples 正反对拍 + generated 重建
2. `server/src/agent_ipc/command_executor.rs` 新 handler（仿 `spawn_event` 资源接线范式，`:344-352`）→ 写 `EnvironmentOverlays`
3. narration 天象预兆（scope=zone，style=perception），文案示例：
   - 「白茫从谷底漫上来。十步之外，人影成灰。」
   - 「雾里一股湿冷的土腥气——今日不宜赶路。」
   - 「有什么东西在雾深处走动，脚步声比你的多一只。」
4. tiandao 推演侧：灾劫/变化 Agent 的工具面注册（mock 模式含该命令的样例输出）

**测试**：schema 正反 sample 双端对拍；executor happy + 非法 density + 过期 TTL 边界；narration 模板 scope/style pin；mock 推演产出含 spawn_fog_bank 的回归样例。

## P3 — 消费方校准 + e2e

- HeavyHaze（0.85 恰在档位起点，t=0 时浓雾档无增益）是否上调至 0.90 —— 见 §8 #1，按决议落数
- runClient 截图基线：浓雾内/边界羽化带/仰视天空三视角；`/fog spawn` → 截图 → `/fog clear` 闭环
- e2e：server 起雾 → bot 收 payload →（可选）client 手动核验；`scripts/smoke-test.sh` 不回归
- 性能：雾堤 + 天气雾 + zone 大气三源共存时 client tick 预算不劣化（沿用 `perfEightConcurrentEffectsInView` 口径加一条浓雾 case）

---

## §8 开放问题（升 active / P0 决策门前需收口）

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

1. **HeavyHaze 数值**：0.85 恰好落在浓雾档起点（t=0，视觉与现状几乎无差）。上调到 0.90 让长阴霾真正压视野，还是保持 0.85 只做"遮蔽阈值触发器"？数值 owner 是 plan-zone-weather-v1，改动需在 PR 内注明跨 plan 数值变更
2. **RealmVision 仲裁语义**：min-combine 是否会让"境界视觉"（看得更远/更清）被浓雾完全吃掉？是否需要"化虚境在浓雾中可视距离 ×1.5"这类境界特权（worldview §三 境界感知差异）？
3. **nameplate / glow 穿雾**：MC nameplate 在 64 格内穿墙渲染，浓雾中会暴露位置。disguise 系统已有按 ViewDistance 半径过滤先例（(vd+2)×16 Chebyshev）——ViewDistance 被压到 16 后是否已天然解决？需实测后定是否要额外处理
4. **游离风暴（worldview §二）**：移动雾堤（AABB 每 tick 平移 + 负压物理）是否 v2 另立 plan？本 plan 静态雾堤是其视觉载体前置。负压吸真元必须走 qi_physics ledger，明确不在本 plan scope
5. **雾堤持久化**：server 重启后雾堤是散掉（不入库，v1 简单）还是随 `bong.db` 恢复（TTL 语义要存绝对 tick）？
6. **wangyintai_atmosphere 半孤儿**：`build_fog_veil()`（density 0.01，纯氛围）生产无调用者。是顺带接进 `default_effects_for_zone_with_profile` 的 wangyintai 分支，还是留给 woliu 后续 plan？本 plan 默认不接，只登记

## §8.1 决议（pre-P0 收口，2026-07-10）

### #1 HeavyHaze 数值

**决议**：
1. 保持 0.85 不动。
2. 浓雾档分段曲线在 0.85 处连续（t=0 时与现状视觉一致），HeavyHaze 的职责是"触发 ViewDistance 遮蔽"而非伸手不见五指；伸手不见五指级由雾堤（`EnvironmentOverlays`，density 可到 1.0）承载。
3. P3 runClient 截图校准后若确认长阴霾压迫感不足，再以单独 PR 上调至 0.90 并在 PR body 注明跨 plan 数值变更（数值 owner `plan-zone-weather-v1`）；本 plan 各阶段不动它。

**落点**：`server/src/world/weather_to_environment.rs:71-83`（HeavyHaze bundle）/ plan §P0-1（分段曲线连续性约束）

### #2 RealmVision 仲裁语义

**决议**：
1. min-combine（fogStart/fogEnd 各取更近者），与 `mergeFogCommands` 同语义；不引入境界视距特权。
2. 实现收敛为单一 GL sink：保留 `MixinFogPerZone` 一处写 `RenderSystem`，`RealmVisionFogController` 的命令并入 `EnvironmentFogController` 仲裁后统一输出，消除同 TAIL 注入点执行序未定义的互覆。
3. 拒绝"化虚穿雾"路线：RealmVision 语义是境界感知染色（plan-perception-v1.1），worldview §三 未定义视距特权，不发明正典；v2 若要做穿雾须先补 worldview。

**落点**：`client/src/main/java/com/bong/client/mixin/MixinBackgroundRendererRealmVision.java:13-26` + `MixinFogPerZone.java:13-29`（同 TAIL 注入点实证）/ `client/.../visual/realm_vision/GlFogParamsSink.java:9-12` / plan §P0-4

### #3 nameplate / glow 穿雾

**决议**：
1. v1 不额外处理。
2. density ≥ 0.85 时 `weather_vision_obscure_system` 已把 ViewDistance 压到 16 格（`vision_obscure_radius`），实体包裁剪随 vd 收缩，disguise 过滤半径 (vd+2)×16 同步缩小——穿帮面天然收窄。
3. P3 实测记录 nameplate 表现；确认穿帮再入 v2，不预支实现。

**落点**：`server/src/world/weather_physics/vision.rs:52-96` / `lingtian/weather_profile.rs:19,66-70` / plan §P3

### #4 游离风暴（移动雾堤）

**决议**：
1. 不在本 plan scope，v2 另立 plan（暂名 plan-drift-storm）。
2. 本 plan 的静态雾堤（AABB + TTL）是其视觉载体前置；移动 = AABB 每 tick 平移，属增量改动。
3. 负压吸真元必须走 `qi_physics::ledger`（worldview §二 L50 游离风暴是负灵域形态），物理耦合是另立 plan 的核心理由——本 plan 保持零 qi_physics 耦合。

**落点**：`docs/worldview.md §二 L50` / plan §8 #4 原表

### #5 雾堤持久化

**决议**：
1. v1 不入库，server 重启即散。
2. 雾堤定位是短时天象（dev 测试 + 天道预兆），`FogBank.remaining_ticks` 用相对 tick 递减，无绝对时钟依赖，天然不需要跨重启恢复；`bong.db` 不加表。
3. 若后续出现"常驻迷雾区"需求，正确形态是 zone 配置（`default_effects_for_zone_with_profile` 分支）而非持久化动态 overlay。

**落点**：`server/src/world/environment_overlay.rs`（「/fog 动态雾堤」PR，见 §9）/ plan §P1-1

### #6 wangyintai_atmosphere 半孤儿

**决议**：
1. 本 plan 不接。
2. `build_fog_veil()` density 0.01 是纯氛围雾，与浓雾档（≥ 0.85）无耦合，接线属 woliu 主题不属雾主题。
3. 归属 woliu 后续 plan；以本节为登记点，归档时抄入 Finish Evidence 遗留清单。

**落点**：`server/src/world/wangyintai_atmosphere.rs:19,56-66` / plan §8 #6 原表

## §9 进度日志

- **2026-07-10**：骨架立项。调研结论：server 遮蔽机制（vision.rs 0.85 阈值 + ViewDistance 压缩）与 wire（FogVeil 任意 AABB）均已就绪，缺口 = client 渲染档（公式钳死 fogEnd≥44 / 天空穿帮 / 边界瞬跳 / 双 sink 互覆）+ server 动态注入面（sync 每帧全量覆盖）+ 天道命令入口（agent_cmd 7 类无环境类）。已查 finished_plans（zone-environment / zone-weather / zone-atmosphere / perception / woliu）、active、skeleton 与 reminder.md，无同主题重叠
- **2026-07-10**：升 active（用户拍板）+ §8.1 六条决议收口（基于两路 Explore 实地调研，文件:行号双锚点齐）。P1 主体（`EnvironmentOverlays` + `/fog` dev 命令 + bot 场景 + 17 单测）由 PR #1158 先行交付，P1 置 ⏳；P0/P2/P3 待 consume
