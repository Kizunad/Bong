# Bong · plan-dense-fog-v1

**区域浓雾（雾堤）**：把 `density ≥ 0.85` 定为跨端统一的「浓雾档入口」——client 渲染进入连续浓雾段（fogEnd 51.8 → 8.0），server 端它同时就是既有的遮蔽阈值（ViewDistance 压缩）；"勉强伸手不见五指"（fogEnd ≈ 8 格）对应 **density = 1.0 的雾堤**。server 提供不被天气 sync 冲掉的**动态雾堤注入源**（任意 AABB + TTL），天道 agent 获得下雾的命令入口。补齐 `plan-zone-environment-v1` 归档遗留第 5 项「Agent → server 注入 API」。

**日期约定**：`YYYY-MM-DD` 均为 Pacific/Auckland 本地日期。

## 为什么不加新 enum 变体（核心设计决策）

- server `weather_physics/vision.rs:11` 已存在 `OPAQUE_FOG_DENSITY_THRESHOLD = 0.85`：玩家所在 zone 的 effects 含 density ≥ 0.85 的 FogVeil 时，`weather_vision_obscure_system` 把该玩家 `ViewDistance` 压到 `chunks_for_radius(vision_obscure_radius)`（默认 16 blocks → **1 chunk**，`vision.rs:132-137` ceil 换算；profile 可 per-zone 覆盖）——**服务端权威的"看不见"已经在跑**（chunk 根本不发，透视外挂也看不穿），天气 `HeavyHaze`（density 0.85，`weather_to_environment.rs:71-83`）已是触发者
- wire `FogVeil { aabb_min, aabb_max, tint_rgb, density }` 本来就支持任意 AABB 子区域（`server/src/world/environment.rs:32-37`），**本 plan 零 schema 漂移**（P2 天道命令除外）
- 缺口全在两头：client 渲染公式钳死在 fogEnd ≥ 44（`EnvironmentFogPlanner.java:32-33`，到不了伸手不见五指）；server 没有绕过每帧 `replace_for_dimension` 全量覆盖（`environment.rs`）的动态注入面
- 复用 density 语义 → HeavyHaze 天气自动落进浓雾档入口（视觉与现状一致，见 §0），雾堤/天道下雾即刻可用同一条 wire

## §0 跨端契约：权威遮蔽 vs 视觉渲染（两条消费线，一个数值）

同一常量 0.85，两端各有明确职责，**互不承诺对方的行为**：

| 消费线 | 判定输入 | 语义 | 落点 |
|---|---|---|---|
| **server 权威遮蔽** | FogVeil **原始 density**（zone-scoped：玩家所在 zone 的 effects 任一命中即触发，与玩家是否在雾 AABB 内无关） | 二值：ViewDistance → `chunks_for_radius(vision_obscure_radius)` | `weather_physics/vision.rs:52-96`（既有，本 plan 不改其逻辑） |
| **client 视觉渲染** | **effective density** `d_eff = density × alpha × edgeFactor`（时间淡入 × 边界羽化） | 连续：雾距曲线与天空遮蔽**同源**用 `d_eff` | P0 交付 |

- 设计意图：权威信息裁剪**不能**依赖客户端连续值（防作弊宁可多裁）；玩家站在羽化带时 chunk 已被裁但雾稍薄——信息不超发，方向安全。这是契约不是事故。
- 命名区分：server `OPAQUE_FOG_DENSITY_THRESHOLD`（遮蔽）、client `DENSE_FOG_TIER_START`（渲染段入口），共享数值 0.85，互锁机制见 P0-5。
- 视觉档位速查（`d_eff` → fogStart/fogEnd）：0.85 → 12.7/51.8（=现状，HeavyHaze 落点）；0.95 → 5.2/22.6；**1.0 → 1.5/8.0（勉强伸手不见五指，雾堤 dev/天道默认按 1.0 下发）**。

**交叉引用**：
- `plan-zone-environment-v1.md`（finished）—— FogVeil 协议 / `ZoneEnvironmentRegistry` / `EnvironmentFogPlanner` 全部由它立，本 plan 是其"浓雾档"延伸 + 遗留第 5 项收口
- `plan-zone-weather-v1.md`（finished）—— `weather_to_environment.rs` 天气→FogVeil 映射表；HeavyHaze/LingMist 数值 owner，P3 若调参须单独 PR 注明跨 plan 数值变更
- `plan-zone-atmosphere-v2.md`（finished）—— `mergeFogCommands` 双源合并语义（`ZoneAtmosphereRenderer.java:105-121`），浓雾档沿用 min-combine 不重造
- `plan-perception-v1.1.md`（finished）—— RealmVision fog sink（`GlFogParamsSink.java:9-12`）与环境雾 sink 同注入点冲突，P0 必须仲裁
- `plan-woliu-path-v1.md`（finished）—— `wangyintai_atmosphere.rs` 的 `build_fog_veil()` 生产无调用者（半孤儿），本 plan **不接**，仅在 §8 登记

**接入面**（防孤岛）：
- **进料**：`ZoneEnvironmentRegistry`（`server/src/world/environment.rs`）；`ActiveWeather`（`lingtian/weather.rs`，HeavyHaze 现成触发者）；`bong:agent_cmd`（P2 新命令类型）；`/fog` dev 命令（P1，已由 PR #1158 交付）
- **出料**：`bong:zone_environment` S2C（channel / 形状均不变）→ client `EnvironmentFogPlanner` 浓雾分段渲染；`weather_vision_obscure_system` ViewDistance 压缩（既有，本 plan 不改其逻辑）；天道 narration 走 `PendingGameplayNarrations`（`server/src/player/gameplay.rs:93` 既有队列，P2）
- **共享类型**：复用 `EnvironmentEffect::FogVeil`（**不新增变体**）、`OPAQUE_FOG_DENSITY_THRESHOLD`、`ZoneWeatherProfile.vision_obscure_radius`、`PendingGameplayNarrations`；client 复用 `EnvironmentFogCommand` / `FogVeilEmitter` / `mist_low_loop`
- **跨仓库契约**：server `EnvironmentOverlays`（PR #1158 已落地）+ `CommandType` 新增 `spawn_fog_bank`（`agent/packages/schema/src/common.ts:29-38` union 扩展 + samples）↔ client `EnvironmentFogPlanner` 浓雾分段 + 天空遮蔽；`bong:zone_environment` payload 不变；跨端阈值互锁走 `contracts/fog-thresholds.json`（P0-5）
- **worldview 锚点**：§八「天道的手段·中等手段——发布天象预兆让修士自行迁移」（浓雾 = 天象预兆的视觉本体）；§二「负灵域·游离风暴」（移动负能风暴的视觉载体，本 plan 只做静态雾堤表现，游离物理见 §8）；§K 信息红线（雾是氛围与信息压制，无任何 HUD 数值/图标）
- **qi_physics 锚点**：**无**——纯表现层 + ViewDistance 压缩，不引入任何衰减/物理常数，不读写 `spirit_qi` / `qi_current`。红旗自检：本 plan 实装代码 grep `DECAY|DRAIN|RHO|BETA` 0 命中

**阶段总览**：
- P0 ⬜ client 浓雾渲染档：分段曲线 + 天空/日月星遮蔽（同源 `d_eff`）+ 自适应边界羽化 + RealmVision sink 仲裁 + 跨端阈值契约文件
- P1 ⏳ server 动态雾堤：`EnvironmentOverlays`（AABB 相交多 zone 附着 + `remaining_ticks` TTL）+ `/fog` dev 命令 + bot 场景（主体已由 PR #1158 交付，见 §9；余：`weather_vision_obscure_system` 集成用例、join 补发含 overlay 用例、跨 zone 附着集成用例）
- P2 ⬜ 天道接线：`spawn_fog_bank` CommandType + executor handler（overlay + `PendingGameplayNarrations`）+ 双端 sample
- P3 ⬜ 消费方校准：runClient 截图基线 + bot 可重复实体可见性验证 + 数值调参 + e2e

---

## P0 — client 浓雾渲染档

**目标**：`d_eff = 1.0` 时 client 真正"勉强伸手不见五指"（fogStart 1.5 / fogEnd 8.0），浓雾段（≥ 0.85）连续渐进；盒心（羽化带以内）既有 zone 视觉不变，边界带行为由"瞬跳"变"渐变"（**预期改进，明确撤销"边界行为完全不变"的承诺**）。

交付物（`client/src/main/java/com/bong/client/environment/` + `mixin/`）：

1. **分段雾距曲线**（`EnvironmentFogPlanner`，输入统一为 `d_eff`）：
   - `d_eff < 0.85`：维持现行 `fogStart = 28 − 18d`，`fogEnd = 96 − 52d`
   - `d_eff ∈ [0.85, 1.0]`：`t = (d_eff − 0.85) / 0.15`；`fogStart = 12.7 − 11.2t`（→ 1.5），`fogEnd = 51.8 − 43.8t`（→ 8.0）。两段在 0.85 处连续；档位速查见 §0
   - 常量 `DENSE_FOG_TIER_START = 0.85`；浓雾段 `FogShape` 由 `CYLINDER` 切 `SPHERE`（头顶不漏光）
2. **天空遮蔽**（`EnvironmentSkyController` + `MixinSkyPerZone`）：遮蔽因子 `o = clamp01((d_eff − 0.85) / 0.10)`——**与雾距同源用 `d_eff`**，边界羽化时天空与雾距一起渐变，不允许"雾距渐变天空瞬跳"；sky shader color 向雾色混合权重从现行 0.45 封顶改为 `max(现行值, o)` 直至全遮；`o ≥ 0.5` 时跳过日月星/云渲染（`renderSky` celestial 段早退）；浓雾段雾色 = `tint_rgb` 全权重
3. **自适应边界羽化**（`EnvironmentFogPlanner`）：替换二值 `contains()` 门——逐轴 `feather_axis = min(12.0, half_extent_axis)`，`edgeFactor = clamp01(min over axes(玩家到该轴两内侧面距离 / feather_axis))`。**任意尺寸 AABB 的中心恒有 edgeFactor = 1**（窄盒不再永远够不到浓雾档）；对既有 zone FogVeil 同样生效（zone AABB 大，羽化只影响边缘 12 格）
4. **fog sink 仲裁**：`EnvironmentFogController` 与 `RealmVisionFogController` 同在 `applyFog` TAIL 且 mixin 优先级相同（注入顺序未定义，实测互覆）。收敛单一 GL sink：保留 `MixinFogPerZone` 一处写 `RenderSystem`，RealmVision 命令并入 min-combine 仲裁（同 `mergeFogCommands` 语义）
5. **跨端阈值契约文件**：新增仓库根 `contracts/fog-thresholds.json`（`{"dense_fog_tier_start": 0.85}`）。Rust 测试 `include_str!` 对拍 `OPAQUE_FOG_DENSITY_THRESHOLD`；client Java 测试读同文件对拍 `DENSE_FOG_TIER_START`。**单边改常量 → 该端红；改 JSON 不同步两端 → 两端红**（真互锁，替代"两端各自 pin 字面值"的伪互锁）
6. **音效/粒子**：复用 `FogVeilEmitter`（密度驱动粒子预算已存在）+ `mist_low_loop` ambient（`EnvironmentEffect.java:216` 已接）；不新增音效协议

**测试**（Java，饱和）：
- 曲线：两段各 3 点 + 0.85 连续性 + d_eff=1.0 极值 pin（1.5/8.0）
- 羽化：盒厚 1 / 8 / 16 / 24 / 96 格五档断言"中心恒达原始 density、羽化带线性、盒外 0"；退化 AABB（零厚/负翻转）不 NaN 不 panic
- 既有雾回归基线：scorch(0.34) / tribulation(0.42) / tsy(0.58) 三 zone 各取 盒心 / 羽化带中点 / 盒外 三采样点 pin（盒心值与现行公式输出一致）
- 天空：o 曲线 0.84 / 0.90 / 0.95 三点 + celestial 跳过分支 + 与雾距同源（同一 d_eff 输入）
- 仲裁：四象限（env 浓 / RV 浓 / 双活 / 双空）
- 契约：`contracts/fog-thresholds.json` 对拍（Rust + Java 各一条）

## P1 — server 动态雾堤 + dev 命令

**目标**：在任意 AABB 上起一片有寿命的浓雾，不被 `weather_environment_sync_system` 每帧 `replace_for_dimension` 冲掉，且**跨 zone 正确分发**。

> 主体已由 PR #1158 交付（`environment_overlay.rs` 9 单测 + `cmd/dev/fog.rs` 8 单测 + bot 场景），以下为完整契约描述 + 剩余交付。

交付物：

1. `server/src/world/environment_overlay.rs`（已落地）：`FogBank { id, dimension, aabb_min/max: [f64;3], tint_rgb: [u8;3], density: f32, remaining_ticks: Option<u64> }` + `EnvironmentOverlays` Resource。**TTL 状态表（唯一模型，`remaining_ticks` 相对递减，无绝对时钟）**：
   | 状态/输入 | 行为 |
   |---|---|
   | spawn（duration=N>0） | `remaining_ticks = Some(N)`；spawn 当 tick 即参与组装 |
   | 每 tick（`sync_zone_environment_effects` 开头 `tick_expiry`） | 饱和递减 1；递减后 = 0 → 当 tick 摘除，**不再参与本 tick 组装** |
   | 净存活时长 | 恰好 N 个 sync tick |
   | duration = 0 | 命令层 / executor 层拒绝（常驻请省略参数） |
   | duration 省略 | `None`，常驻直到显式 clear |
   | 摘除/清除后的广播 | 靠组装结果变化 → `replace_for_dimension` diff → dirty 重播（无独立 mark 路径） |
   | server 重启 | 全散（不入库，§8.1 #5） |
2. **zone 附着 = AABB 相交集合**（已落地，`fog_effects_for_zone`）：雾堤附着到**所有** dimension 相同且 AABB 相交（闭区间）的 zone——跨两 zone 的雾堤进入两个 zone 的 state、join 补发和 `vision_obscure` 读取路径；同一雾堤在多 zone state 中重复出现是**预期**（client planner 按 contains + max 择一，值相同无副作用）。不与任何 zone 相交 → 命令/executor 拒绝（广播是 zone-scoped，登记了也发不出去）
3. `/fog` dev 命令（已落地，`server/src/cmd/dev/fog.rs`）：`spawn <radius> <density> [duration_ticks]`（以执行者为中心，竖直跨度 −24/+48）、`clear <id>`、`clear_all`、`list`；registry_pin 5 条路径。~~CLAUDE.md dev 命令表~~——已由 PR #1158 **人工**交付；CLAUDE.md 不在 consume-plan 写权限内，**不列为本 plan 自动交付物**
4. bot 场景（已落地）：`scripts/bot/scenarios/cmd_dev_fog_zone_environment.py`——spawn 后广播含 density=0.93 fog_veil、clear_all 后同 zone 重播不含
5. **剩余交付**：
   - `weather_vision_obscure_system` 集成用例：dev 雾堤 density ≥ 0.85 → 同 zone 客户端 ViewDistance 被压到 `chunks_for_radius`；< 0.85 不压；雾堤到期后恢复
   - 新客户端 join 补发用例：`mark_all_dirty_for_snapshot` 快照含 overlay 产生的 FogVeil
   - 跨 zone 附着集成用例：跨两 zone / 中心在 wilderness 但边缘相交 / 多区重叠 / 两名玩家位于不同 zone 同一雾堤内各自收到含该雾堤的 state

**测试**（Rust，饱和；已落地 17 条 + 剩余交付各至少 1 条）：overlay merge 进组装 / 与天气 FogVeil 共存 / TTL 状态表逐行（含 0 拒绝、1 tick、常驻）/ 跨 dimension 隔离 / 贴边闭区间 / `/fog` 三子命令 happy + 非法分支全覆盖 / vision_obscure 集成 / join 补发 / 跨 zone 附着。

## P2 — 天道接线（补 zone-environment-v1 遗留第 5 项）

**目标**：天道 agent 能以「天象预兆」形式下雾（worldview §八 中等手段），narration 有真实的玩家可见消费链路。

交付物：

1. `agent/packages/schema/src/common.ts` `CommandType` union 新增 `"spawn_fog_bank"`；`agent-command.ts` 参数：`{ aabb_min, aabb_max, tint_rgb?, density, duration_ticks, narration? }`；samples 正反对拍 + generated 重建
2. `server/src/network/command_executor.rs` 新 handler `execute_spawn_fog_bank`：校验（density ∈ (0,1] / duration > 0 / AABB 与至少一个 zone 相交，同 P1 规则）→ 写 `EnvironmentOverlays`
3. **narration 消费链路（完整 symbol 链）**：`bong:agent_cmd` → `execute_spawn_fog_bank` → `narration` 字段非空时写 `PendingGameplayNarrations`（`server/src/player/gameplay.rs:93`，既有队列 + 既有 flush→client 路径），定向 = 命中 zone 内玩家，style=perception，scope=zone；**缺省/空串 = 不发 narration，只下雾**。文案示例：
   - 「白茫从谷底漫上来。十步之外，人影成灰。」
   - 「雾里一股湿冷的土腥气——今日不宜赶路。」
   - 「有什么东西在雾深处走动，脚步声比你的多一只。」
4. tiandao 推演侧：灾劫/变化 Agent 工具面注册；mock 模式含 spawn_fog_bank 样例输出

**测试**：schema 正反 sample 双端对拍；executor happy + 非法 density / duration=0 / AABB 无 zone 相交三拒绝分支；**narration 集成**（agent_cmd 输入 → `PendingGameplayNarrations` 出现定向该 zone 的条目；narration 缺省分支 → 队列无新条目）；mock 推演回归样例。

## P3 — 消费方校准 + e2e

- HeavyHaze 是否从 0.85（档位入口，视觉与现状一致）上调至 0.90——按 §8.1 #1，默认不动；确需调整走单独 PR 注明跨 plan 数值变更
- runClient 截图基线：雾堤内（d=1.0）/ 羽化带 / 仰视天空三视角；`/fog spawn 48 1.0` → 截图 → `/fog clear_all` 闭环
- **实体可见性可重复验证**（替代截图印象）：bot 场景断言浓雾 zone 内，超出 `chunks_for_radius(vision_obscure_radius)` 的实体不再收到 spawn/更新包（参照 `network_disguise_visibility_scoping.py` 先例）；nameplate 结论以此为准（§8.1 #3）
- e2e：server 起雾 → bot 收 payload → `scripts/smoke-test.sh` 不回归
- 性能：雾堤 + 天气雾 + zone 大气三源共存时 client tick 预算不劣化（沿用 `perfEightConcurrentEffectsInView` 口径加一条浓雾 case）

---

## §8 开放问题（升 active / P0 决策门前需收口）

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

1. **HeavyHaze 数值**：0.85 恰好落在浓雾档起点（t=0，视觉与现状几乎无差）。上调到 0.90 让长阴霾真正压视野，还是保持 0.85 只做"遮蔽阈值触发器"？数值 owner 是 plan-zone-weather-v1，改动需在 PR 内注明跨 plan 数值变更
2. **RealmVision 仲裁语义**：min-combine 是否会让"境界视觉"（看得更远/更清）被浓雾完全吃掉？是否需要"化虚境在浓雾中可视距离 ×1.5"这类境界特权（worldview §三 境界感知差异）？
3. **nameplate / glow 穿雾**：MC nameplate 在 64 格内穿墙渲染，浓雾中会暴露位置。disguise 系统已有按 ViewDistance 半径过滤先例（(vd+2)×16 Chebyshev）——ViewDistance 被压缩后是否已天然解决？需实测后定是否要额外处理
4. **游离风暴（worldview §二）**：移动雾堤（AABB 每 tick 平移 + 负压物理）是否 v2 另立 plan？本 plan 静态雾堤是其视觉载体前置。负压吸真元必须走 qi_physics ledger，明确不在本 plan scope
5. **雾堤持久化**：server 重启后雾堤是散掉（不入库，v1 简单）还是随 `bong.db` 恢复（TTL 语义要存绝对 tick）？
6. **wangyintai_atmosphere 半孤儿**：`build_fog_veil()`（density 0.01，纯氛围）生产无调用者。是顺带接进 `default_effects_for_zone_with_profile` 的 wangyintai 分支，还是留给 woliu 后续 plan？本 plan 默认不接，只登记

## §8.1 决议（pre-P0 收口，2026-07-10）

### #1 HeavyHaze 数值

**决议**：
1. 保持 0.85 不动。
2. 0.85 = 浓雾档**入口**：HeavyHaze 的职责是触发 ViewDistance 遮蔽，视觉与现状一致（分段曲线在 0.85 连续，t=0）；"勉强伸手不见五指"由 density = 1.0 的雾堤承载（§0 档位速查）。
3. P3 runClient 截图校准后若确认长阴霾压迫感不足，再以单独 PR 上调至 0.90 并注明跨 plan 数值变更（数值 owner `plan-zone-weather-v1`）；本 plan 各阶段不动它。

**落点**：`server/src/world/weather_to_environment.rs:71-83`（HeavyHaze bundle）/ plan §0 档位速查 + §P0-1

### #2 RealmVision 仲裁语义

**决议**：
1. min-combine（fogStart/fogEnd 各取更近者），与 `mergeFogCommands` 同语义；不引入境界视距特权。
2. 实现收敛为单一 GL sink：保留 `MixinFogPerZone` 一处写 `RenderSystem`，`RealmVisionFogController` 的命令并入 `EnvironmentFogController` 仲裁后统一输出，消除同 TAIL 注入点执行序未定义的互覆。
3. 拒绝"化虚穿雾"路线：RealmVision 语义是境界感知染色（plan-perception-v1.1），worldview §三 未定义视距特权，不发明正典；v2 若要做穿雾须先补 worldview。

**落点**：`client/src/main/java/com/bong/client/mixin/MixinBackgroundRendererRealmVision.java:13-26` + `MixinFogPerZone.java:13-29`（同 TAIL 注入点实证）/ `client/.../visual/realm_vision/GlFogParamsSink.java:9-12` / plan §P0-4

### #3 nameplate / glow 穿雾

**决议**：
1. v1 不额外做 nameplate 特殊处理。
2. density ≥ 0.85 时 `weather_vision_obscure_system` 把 ViewDistance 压到 `chunks_for_radius(vision_obscure_radius)`（默认 16 blocks → 1 chunk），实体包裁剪随 vd 收缩，disguise 过滤半径 (vd+2)×16 blocks 同步缩小——穿帮面天然收窄。
3. 结论以 P3 的 bot 可重复验证为准（断言超出遮蔽半径的实体不收 spawn/更新包），不依赖手工截图印象；确认穿帮再入 v2。

**落点**：`server/src/world/weather_physics/vision.rs:31-45,52-96,132-137`（blocks→chunks ceil 换算）/ `lingtian/weather_profile.rs:31,66-70` / plan §P3

### #4 游离风暴（移动雾堤）

**决议**：
1. 不在本 plan scope，v2 另立 plan（暂名 plan-drift-storm）。
2. 本 plan 的静态雾堤（AABB + TTL）是其视觉载体前置；移动 = AABB 每 tick 平移，属增量改动。
3. 负压吸真元必须走 `qi_physics::ledger`（worldview §二 L50 游离风暴是负灵域形态），物理耦合是另立 plan 的核心理由——本 plan 保持零 qi_physics 耦合。

**落点**：`docs/worldview.md §二 L50` / plan §8 #4 原表

### #5 雾堤持久化

**决议**：
1. v1 不入库，server 重启即散。
2. TTL 唯一模型 = `FogBank.remaining_ticks: Option<u64>` 相对递减（状态表见 §P1-1），无绝对时钟依赖，天然不需要跨重启恢复；`bong.db` 不加表。
3. 若后续出现"常驻迷雾区"需求，正确形态是 zone 配置（`default_effects_for_zone_with_profile` 分支）而非持久化动态 overlay。

**落点**：`server/src/world/environment_overlay.rs`（PR #1158）/ plan §P1-1 状态表

### #6 wangyintai_atmosphere 半孤儿

**决议**：
1. 本 plan 不接。
2. `build_fog_veil()` density 0.01 是纯氛围雾，与浓雾档（≥ 0.85）无耦合，接线属 woliu 主题不属雾主题。
3. 归属 woliu 后续 plan；以本节为登记点，归档时抄入 Finish Evidence 遗留清单。

**落点**：`server/src/world/wangyintai_atmosphere.rs:19,56-66` / plan §8 #6 原表

## §9 进度日志

- **2026-07-10**：骨架立项。调研结论：server 遮蔽机制（vision.rs 0.85 阈值 + ViewDistance 压缩）与 wire（FogVeil 任意 AABB）均已就绪，缺口 = client 渲染档（公式钳死 fogEnd≥44 / 天空穿帮 / 边界瞬跳 / 双 sink 互覆）+ server 动态注入面（sync 每帧全量覆盖）+ 天道命令入口（agent_cmd 7 类无环境类）。已查 finished_plans（zone-environment / zone-weather / zone-atmosphere / perception / woliu）、active、skeleton 与 reminder.md，无同主题重叠
- **2026-07-10**：升 active（用户拍板）+ §8.1 六条决议收口（基于两路 Explore 实地调研，文件:行号双锚点齐）。P1 主体（`EnvironmentOverlays` + `/fog` dev 命令 + bot 场景 + 17 单测）由 PR #1158 先行交付，P1 置 ⏳；P0/P2/P3 待 consume
- **2026-07-10**：按 PR #1156 首轮 /review（4 reviewer 全 REQUEST_CHANGES）修订：新增 §0 拆分「权威遮蔽（server，原始 density，zone-scoped 二值）vs 视觉渲染（client，effective density 连续）」双消费线契约；羽化改自适应 `feather_axis = min(12, half_extent_axis)`（窄盒中心恒达原始 density）并撤销"边界行为完全不变"承诺（改为盒心不变 + 三 zone 回归基线）；天空遮蔽与雾距同源用 `d_eff`；TTL 统一为 `remaining_ticks` 单模型 + 状态表；zone 附着从"中心命中"改为"AABB 相交集合"（与 PR #1158 实现一致）；跨端阈值互锁改 `contracts/fog-thresholds.json` 真对拍；narration 补 `PendingGameplayNarrations` 完整消费链；CLAUDE.md 行移出 consume 自动交付物；ViewDistance 单位改为 blocks→chunks ceil 精确表述
