# Bong · plan-zone-environment-v1

**zone-scoped 持续视觉 / 音效 / 局部物理 hook 协议**。把"龙卷风、漂灰、雷柱、雾笼、热浪、晶莹蒸汽"这类**长时间持续**且**空间分布**（柱形 / 球形 / AABB / SDF 切片）的环境效果，做成 zone 级别的**第一公民**。

`plan-vfx-v1` + `plan-particle-system-v1` 已搭好"一次性 VFX 事件"基建（`SpawnParticle.duration_ticks ≤ 200` = 10s 上限）；常态环境效果按 plan-particle-system-v1 §2.4 原则归"客户端自演 / 不广播"。本 plan **补齐**第三类：**zone-scoped 长时持续效果**——server 推 zone state，client 按 state 自演 emitter，进出 zone 平滑过渡。是 `plan-zone-weather-v1` 的硬前置。

**世界观锚点**：
- `worldview.md §六 真元染色谱`（**视觉 ≠ 真元**：本 plan 只动视觉/音效，绝不旁路修改 qi_physics 物理量）
- `worldview.md §八 天道激烈手段 / 天道情绪`（雷暴、灾劫、域崩需要 zone 级视觉载体——光打雷不下雨太单薄）
- `worldview.md §十三 区域详情`（每个 zone 都该有可识别氛围：青云灵境的雾光 / 血谷的腥红雾 / 北荒的冰晶气流 / 焦土的漂灰）
- `worldview.md §十七 末法节律`（季节相位通过 environment effect 间接表达——汐转期天空异常、夏季热浪、冬季雪雾）
- `worldview.md §K 信息红线 · 完全不显式`（环境效果仅为氛围，不携带数值 tooltip / debuff icon）

**library 锚点**：待补 `ecology-XXXX 末法异象录`（zone 标志性气象 / 视觉异象图鉴）。

**交叉引用**：
- `plan-vfx-v1.md`（finished）—— A2 HUD 叠色 + B 系列粒子/实体/BE 渲染基类，本 plan **不重造**渲染基类
- `plan-particle-system-v1.md`（finished）—— `BongLineParticle` / `BongRibbonParticle` / `BongGroundDecalParticle` + `bong:vfx_event` 协议；本 plan §2.4 "环境氛围类客户端自演"原则被本 plan 正式化为 zone state 驱动
- `plan-perception-v1.1.md`（finished）—— `RealmVisionParams` + `MixinBackgroundRendererRealmVision` 已有"按状态改 fog"的 mixin 范式；本 plan 扩到 per-zone tint
- `plan-audio-v1.md`（finished）—— `SoundRecipe` + `loop_while_in_zone` 是本 plan ambient 音效的载体（**不**新建音效协议）
- `plan-lingtian-weather-v1.md`（finished）—— 提供 zone-scoped `ActiveWeather` 数据层，本 plan 不依赖但**为 plan-zone-weather-v1 充当下游消费方**
- `plan-zone-weather-v1.md`（**本 plan 直接下游**，待立）—— 把 `WeatherEvent` 映射到 `EnvironmentEffect`
- `plan-terrain-tribulation-scorch-v1.md`（skeleton）—— 第一个真实消费方：焦土漂灰 + 雷柱 + 黑红天空
- `plan-tribulation-v1.md`（finished）—— 渡劫期可临时注入 environment effect（劫云压盖、紫电闪烁）
- `plan-tsy-v1.md`（finished）—— 末法残土 zone 的"绝对荒芜"氛围可由本 plan 提供（fog tint + 死寂粒子）

**接入面**（防孤岛）：
- **进料**：`server/src/world/zone.rs::Zone`（aabb / id 已存在）；`bong:weather_event_update` Redis pub（plan-zone-weather 消费方注入）；`plan-tribulation` 渡劫事件（注入临时 effect）
- **出料**：客户端 `EnvironmentEffectRegistry` 渲染（粒子 + mixin + 音效）；server 暴露 `EnvironmentPhysicsHook` trait 给消费方（lightning spawn / push velocity / vision obscure），**hook 本身不实装伤害/位移**——消费方自实
- **共享类型**：复用 `cultivation::components::Realm`（perception 染色查询）、`zone::ZoneId`（String）、`plan-audio` 的 `SoundRecipeId`；**不**新建近义 enum
- **跨仓库契约**：server `ZoneEnvironmentState` Resource ↔ agent schema `ZoneEnvironmentStateV1` ↔ client `EnvironmentEffectController`；CustomPayload `bong:zone_environment` (S2C broadcast on change)
- **qi_physics 锚点**：本 plan **不引入**任何真元/灵气物理常数；视觉强度 / 范围 / 持续时间均为表演参数。**红旗自检**：grep `*_DECAY*` / `*_DRAIN*` / `RHO` / `BETA` 应在本 plan 实装代码内为 0 命中。
- **守恒律自检**：environment effect 不写入 `cultivation.qi_current` / `zone.spirit_qi` 任何字段——本 plan 是**纯表现层**，不参与守恒账本。

**阶段总览**：
- P0 ⬜ 协议层 + `EnvironmentEffect` enum 首批 4 变体 + Server `ZoneEnvironmentRegistry` Resource + Schema 双端镜像 + Bevy lifecycle event
- P1 ⬜ 客户端 `EmitterBehavior` trait + 首批 4 emitter（TornadoColumn / LightningPillar / AshFall / FogVeil）+ 玩家距离 culling + 进出 zone 淡入淡出
- P2 ⬜ Mixin 扩展（`MixinFogPerZone` + `MixinSkyPerZone`）+ ambient audio 联动（plan-audio `loop_while_in_zone`）+ `EnvironmentPhysicsHook` trait 暴露
- P3 ⬜ 剩余 emitter（DustDevil / EmberDrift / HeatHaze / SnowDrift）+ scorch / tribulation / tsy 三个真实消费方接入示例 + 性能压测（同时 N 个 effect / 玩家可见区域 emit budget）

---

## §0 设计轴心

- [ ] **server 推状态，client 自演 emitter**——不走 `bong:vfx_event` 一次性事件协议（避免每秒数百包）；走 `bong:zone_environment` **状态广播**（仅在 effect 列表变化时推），client 按状态持续 emit
- [ ] **state diff broadcast，不是全量**——effect 列表只在 `add / remove / param-change` 时推，进出 zone 由 client 自决渲染（client 已有玩家位置）
- [ ] **emitter 玩家距离 culling**：client 仅在玩家与 effect bbox 中心 < `view_radius`（首版 80 块，可 per-effect 配置）时 emit；超出后停止 emit + 平滑淡出
- [ ] **进出 zone 淡入淡出**：玩家穿越 zone 边界时 effect 强度 0 → 1 在 N tick 内插值（首版 40 tick = 2s）
- [ ] **emitter 是渲染概念，不是物理概念**——effect 携带空间形状（AABB / Cylinder / Sphere），但**不参与碰撞 / 不影响修炼**；物理后果由消费方通过 `EnvironmentPhysicsHook` 注入（消费方拿 effect 的形状自己写碰撞 / push）
- [ ] **复用 plan-particle-system-v1 渲染基类**：`BongLineParticle` / `BongRibbonParticle` / 等；本 plan 不新建粒子基类
- [ ] **复用 plan-audio-v1 ambient loop**：每个 effect 可选关联一个 `SoundRecipeId`，client 按 effect 状态启停 loop（不新建音效协议）
- [ ] **mixin 扩展遵循 plan-perception §0 "Planner / Mixin 分层"**：业务逻辑（哪些 zone 该改 fog）在纯函数 Planner，Mixin 只做"Planner 输出 → RenderSystem 转发"
- [ ] **绝不破 worldview §K 红线**：environment effect 没有 tooltip / icon / 数值显示——是氛围而非 UI

---

## §1 EnvironmentEffect 类型清单（首版 8 变体）

| 变体 | 形状 | 视觉路径 | Audio loop（plan-audio） | 典型消费方 |
|---|---|---|---|---|
| `TornadoColumn { center, radius, height, particle_density }` | Cylinder | **分层环 cloud256 大 billboard**（30-50 层 × 每层 ~30 个 SpriteBillboardParticle，scale 3-8 块，α≈0.25 叠"实心"）+ 少量 RibbonParticle 外延骨架（参考 Weather2 `TornadoFunnelSimple.LayerSpec`） | `wind_howl_loop` | scorch / 北荒 / 沙漠 zone |
| `LightningPillar { center, radius, strike_rate_per_min }` | Cylinder（细） | 周期性 lightning_bolt entity + ember 粒子 | `thunder_distant_loop` | scorch / tribulation 渡劫 |
| `AshFall { aabb, density }` | AABB | vanilla `falling_dust` + 自定义 `bong:ash` | `static_crackle_loop` | scorch / 末法 tsy |
| `FogVeil { aabb, tint_rgb, density }` | AABB | mixin fog tint + 自定义 GroundDecalParticle | `mist_low_loop` | 阴霾 / 血谷 / 灵雾 |
| `DustDevil { center, radius, height }` | Cylinder（小） | RibbonParticle 螺旋（弱） + vanilla `large_smoke` | `wind_dry_loop` | DroughtWind 旱风 |
| `EmberDrift { aabb, density, glow }` | AABB | LineParticle 上飘 + 发光 | `static_crackle_loop` | scorch 雷磁柱周围 / 渡劫 |
| `HeatHaze { aabb, distortion_strength }` | AABB | mixin 屏幕扭曲（轻度，复用 perception 范式）| `cicada_summer_loop` | 夏季热浪 / scorch |
| `SnowDrift { aabb, density, wind_dir }` | AABB | vanilla `snowflake` + 自定义 LineParticle | `wind_cold_loop` | 北荒 / Blizzard 风雪 |

> **形状语义**：Cylinder 用于"垂直柱状"（龙卷 / 雷柱 / 沙尘暴）；AABB 用于"扁平大区"（雾笼 / 飘灰 / 雪原）；Sphere 留给后续（爆炸冲击波 / 阵法力场）。

---

## §2 数据契约（下游 grep 抓手）

### 2.1 Server (Rust)

```rust
// server/src/world/environment.rs（新文件）
use serde::{Deserialize, Serialize};
use bevy::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvironmentEffect {
    TornadoColumn { center: [f64; 3], radius: f64, height: f64, particle_density: f32 },
    LightningPillar { center: [f64; 3], radius: f64, strike_rate_per_min: f32 },
    AshFall { aabb_min: [f64; 3], aabb_max: [f64; 3], density: f32 },
    FogVeil { aabb_min: [f64; 3], aabb_max: [f64; 3], tint_rgb: [u8; 3], density: f32 },
    DustDevil { center: [f64; 3], radius: f64, height: f64 },
    EmberDrift { aabb_min: [f64; 3], aabb_max: [f64; 3], density: f32, glow: f32 },
    HeatHaze { aabb_min: [f64; 3], aabb_max: [f64; 3], distortion_strength: f32 },
    SnowDrift { aabb_min: [f64; 3], aabb_max: [f64; 3], density: f32, wind_dir: [f32; 3] },
}

#[derive(Debug, Clone, Default, Resource)]
pub struct ZoneEnvironmentRegistry {
    by_zone: HashMap<String, Vec<EnvironmentEffect>>,
    /// 标记需要在下个 broadcast tick 推送的 zone（diff 而非全量）
    dirty: HashSet<String>,
}

impl ZoneEnvironmentRegistry {
    pub fn add(&mut self, zone: impl Into<String>, effect: EnvironmentEffect);
    pub fn remove(&mut self, zone: &str, effect_match: impl Fn(&EnvironmentEffect) -> bool);
    pub fn replace(&mut self, zone: impl Into<String>, effects: Vec<EnvironmentEffect>);
    pub fn current(&self, zone: &str) -> &[EnvironmentEffect];
    pub fn drain_dirty(&mut self) -> Vec<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Event)]
pub enum ZoneEnvironmentLifecycleEvent {
    EffectAdded { zone: String, index: usize },
    EffectRemoved { zone: String, index: usize },
    Replaced { zone: String },
}

pub fn zone_environment_broadcast_system(/* ... */) {
    // drain_dirty → emit RedisOutbound::ZoneEnvironmentUpdate + S2C CustomPayload
}

/// 消费方注入物理后果的 trait——本 plan 不实装任何 hook 实体
pub trait EnvironmentPhysicsHook: Send + Sync {
    fn on_effect_active(&self, effect: &EnvironmentEffect, app: &mut World);
}
```

### 2.2 Schema（agent ↔ server / server ↔ client）

```typescript
// agent/packages/schema/src/zone-environment.ts（新文件）
export const EnvironmentEffectV1 = Type.Union([
  Type.Object({ kind: Type.Literal("tornado_column"), center: Vec3, radius: Type.Number(), height: Type.Number(), particle_density: Type.Number() }),
  Type.Object({ kind: Type.Literal("lightning_pillar"), center: Vec3, radius: Type.Number(), strike_rate_per_min: Type.Number() }),
  Type.Object({ kind: Type.Literal("ash_fall"), aabb_min: Vec3, aabb_max: Vec3, density: Type.Number() }),
  Type.Object({ kind: Type.Literal("fog_veil"), aabb_min: Vec3, aabb_max: Vec3, tint_rgb: Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()]), density: Type.Number() }),
  Type.Object({ kind: Type.Literal("dust_devil"), center: Vec3, radius: Type.Number(), height: Type.Number() }),
  Type.Object({ kind: Type.Literal("ember_drift"), aabb_min: Vec3, aabb_max: Vec3, density: Type.Number(), glow: Type.Number() }),
  Type.Object({ kind: Type.Literal("heat_haze"), aabb_min: Vec3, aabb_max: Vec3, distortion_strength: Type.Number() }),
  Type.Object({ kind: Type.Literal("snow_drift"), aabb_min: Vec3, aabb_max: Vec3, density: Type.Number(), wind_dir: Vec3 }),
]);

export const ZoneEnvironmentStateV1 = Type.Object({
  zone_id: Type.String(),
  effects: Type.Array(EnvironmentEffectV1),
  generation: Type.Integer({ minimum: 0 }),
});
```

### 2.3 Client (Java / Fabric)

```java
// client/src/main/java/.../environment/EnvironmentEffectRegistry.java
public final class EnvironmentEffectRegistry {
    public void onZoneStateUpdate(ZoneEnvironmentStateV1 state);
    public void registerBehavior(String kind, EmitterBehavior behavior);
    public Collection<ActiveEmitter> activeNearPlayer(Vec3d playerPos, double radius);
}

public interface EmitterBehavior {
    void onTickInRadius(MatrixStack stack, Vec3d playerPos, EnvironmentEffect effect, float deltaTick);
    @Nullable SoundRecipeId ambientLoopRecipe(EnvironmentEffect effect);
    /** 进入半径时 0 → 1，离开时 1 → 0 平滑插值 */
    int fadeInTicks();
    int fadeOutTicks();
}

// 8 个内置 EmitterBehavior 实现：TornadoEmitter / LightningPillarEmitter / ...
```

### 2.4 数据契约表

| 契约 | 位置 |
|---|---|
| `EnvironmentEffect` enum (8 变体) | `server/src/world/environment.rs` |
| `ZoneEnvironmentRegistry` Resource | 同上 |
| `ZoneEnvironmentLifecycleEvent` Bevy event | 同上 |
| `EnvironmentPhysicsHook` trait | 同上 |
| `zone_environment_broadcast_system` | `server/src/network/zone_environment_bridge.rs`（新） |
| `EnvironmentEffectV1` / `ZoneEnvironmentStateV1` schema | `agent/packages/schema/src/zone-environment.ts` + Rust 镜像 `server/src/schema/zone_environment.rs` |
| `EnvironmentEffectRegistry` / `EmitterBehavior` | `client/src/main/java/.../environment/` |
| 8 个 emitter 实现 | `client/src/main/java/.../environment/emitter/` |
| `MixinFogPerZone` / `MixinSkyPerZone` | `client/src/main/java/.../mixin/` |
| Redis pub: `bong:zone_environment_update` | server → agent ↔ client |
| CustomPayload: `bong:zone_environment` | server → client (S2C) |

---

## §3 实施节点

- [ ] **P0** —— 协议层 + 4 变体 + 双端 schema + Bevy event
  - 验收：`EnvironmentEffect` 4 变体 (TornadoColumn / LightningPillar / AshFall / FogVeil) Serde + Sample 双端对拍；`ZoneEnvironmentRegistry::{add,remove,replace,drain_dirty}` 单测 ≥ 12 条覆盖每变体 + 重复 add / 跨 zone 隔离 / dirty 清单去重；`zone_environment_broadcast_system` 发 RedisOutbound + S2C payload

- [ ] **P1** —— 客户端 EmitterBehavior + 4 emitter + 距离 culling + 淡入淡出
  - 验收：`EnvironmentEffectRegistry` 注册 4 emitter 单测；玩家在 effect 半径 80 内时 `activeNearPlayer` 返回；超出后返回空；进出 effect bbox 时 fade 0↔1 在 40 tick 内完成；vitest / Java 测试 ≥ 8 条
  - **不**触碰 mixin，**不**接 audio，**不**接物理 hook

- [ ] **P2** —— Mixin 扩展 + audio 联动 + EnvironmentPhysicsHook trait 暴露
  - 验收：`MixinFogPerZone` 接收 FogVeil effect → BackgroundRenderer fog tint 改写（沿用 perception §0 Planner / Mixin 分层模式）；`MixinSkyPerZone` 处理 SkyTint（首版可由 FogVeil 复用）；ambient audio：FogVeil → `mist_low_loop` 自动启动 / 离开 zone 自动停止；`EnvironmentPhysicsHook` trait 公开但本 plan 不实装

- [ ] **P3** —— 剩余 4 emitter + 真实消费方接入 + 性能压测
  - 验收：DustDevil / EmberDrift / HeatHaze / SnowDrift 全部 emitter 接入；scorch / tribulation / tsy 各自至少 1 个真实 effect 在地图内激活；同时 8 个 effect 在玩家 80 块内时 client tick 不掉到 < 50 fps（大致目标）；同 zone 多 effect 不冲突

---

## §4 测试饱和（CLAUDE.md 饱和化测试）

### P0（≥ 12 单测）
- `effect_serde_round_trip_each_variant`（8 条，每变体 1）
- `registry_add_then_current`
- `registry_remove_by_match_predicate`
- `registry_replace_overrides_existing`
- `registry_dirty_drain_idempotent`
- `lifecycle_event_added_removed_pair`

### P1（≥ 8 测）
- `emitter_registers_4_built_in_behaviors`
- `active_near_player_within_radius`
- `active_near_player_outside_radius_empty`
- `fade_in_interpolates_0_to_1_over_n_ticks`
- `fade_out_after_leaving_radius`
- `tornado_column_emit_does_not_throw_at_zero_density`
- `fog_veil_aabb_culling_at_corners`
- `effect_disappears_when_zone_state_replaced`

### P2（≥ 6 测）
- `fog_planner_returns_tint_inside_fog_veil_aabb`
- `fog_planner_returns_default_outside_aabb`
- `sky_planner_blends_two_overlapping_zones`（确定性策略：取 generation 较大者）
- `audio_loop_starts_when_player_enters_fog_veil`
- `audio_loop_stops_after_player_leaves`
- `physics_hook_trait_object_safety`（编译期 trait object 化）

### P3（≥ 5 集成）
- `scorch_zone_full_environment_load`（FogVeil + AshFall + EmberDrift 同 zone）
- `tribulation_inject_lightning_pillar_during_ascension`
- `tsy_zone_dead_silence_fog_veil`
- `multi_zone_overlap_resolved_by_generation`
- `perf_eight_concurrent_effects_in_view`（性能门，记录基线）

---

## §5 开放问题

- [ ] **vanilla rain renderer 替换**：vanilla 雨/雪是全局渲染。如果未来 zone-weather 想做"雨只在 zone 内下"，需 mixin `WeatherRendering` 强制按 zone aabb 裁剪——v1 **不做**，留 v2
- [ ] **多 zone 重叠时 effect 优先级**：玩家同时在两个 FogVeil zone（生成树或 zone 边界毛刺），fog tint 怎么 blend？首版按 `generation` 单调取最新，后续可改 alpha-blend
- [ ] **物理 hook 是否纳入本 plan**：当前设计是 trait 暴露不实装；scorch lightning_strike 应该在 scorch plan 实现，还是统一进 zone-weather plan？倾向**留给消费方**（zone-environment 是纯表现层）
- [ ] **performance budget**：8 个 emitter 同时玩家附近的 emit 上限？参考 plan-particle-system-v1 §2.5（同 tick 同 event 距离 <1m 合批）；本 plan 应跟进
- [ ] **emitter 是否能从 Agent / 天道 推送**：例：天道情绪触发临时 environment effect（如"天怒 → 全图天空紫红 30 min"）？需要 agent → server 注入 API，本 plan **暴露**但**不实装** Agent 写口
- [ ] **client-only emitter**：纯本地的环境效果（如玩家阅读古卷时背景墨色雾起），是否走本协议？倾向**否**——本协议是 server-authoritative 的 zone state，本地视觉走 plan-vfx-v1 / VisualEffectController
- [ ] **EnvironmentEffect 与 BiomeKind 的关系**：是否每 biome 默认带一个固定 effect？倾向**否**——biome 影响生态，environment 影响视觉，两者解耦，由 zone 配置同时声明
- [ ] **cloud256 噪声贴图来源**：`TornadoColumn` / `DustDevil` / `SnowDrift` / `FogVeil` 都依赖 256px 灰云噪声贴图作 SpriteBillboardParticle 的 sprite。来源选项：CC0（OpenGameArt 有现成云噪声）/ AI 生成（项目 `/gen-image particle` skill）/ 自制 PNG。与 `plan-particle-system-v1 §7` 第 1 项 "贴图来源 / 自制 vs 用现成 CC0 资源" 合并处理。本 plan P0 阶段需选定来源并入仓 `client/src/main/resources/assets/bong/textures/particle/`

---

## §6 进度日志

- **2026-05-08**：骨架立项。起因：`plan-terrain-tribulation-scorch-v1` §8 "plan-weather-zone-override 未来再立"被 `plan-lingtian-weather-v1`（2026-05-08 finished）的 `ActiveWeather` zone-scoped 能力部分覆盖，但缺**视觉层** zone-scoped 协议（vfx-v1 是一次性事件）。本 plan 补齐第三类 VFX（zone 长时持续）；`plan-zone-weather-v1` 依赖之。
  - 不与现有任何 skeleton / active plan 同主题重叠（已查 docs/plans-skeleton/ + reminder.md，无 environment / vfx-zone / ambient-zone 类骨架）
  - 升 active 触发条件：（a）`plan-zone-weather-v1` 骨架立项并对齐数据契约；（b）`plan-terrain-tribulation-scorch-v1` 骨架同步更新 P2 视觉路径指向本 plan；（c）首批 emitter 形状完成评审（Cylinder + AABB 两形状是否够用）
- **2026-05-09**：升 active（`git mv docs/plans-skeleton/plan-zone-environment-v1.md → docs/plan-zone-environment-v1.md`）。触发条件核验：
  - (a) `plan-zone-weather-v1` 骨架立项并对齐数据契约 ✅（2026-05-08 立项，§3 已注明依赖本 plan §2 EnvironmentEffect enum）
  - (b) `plan-terrain-tribulation-scorch-v1` P2 视觉路径同步指向本 plan ✅（line 160 / 273 / 314 三处已重定向）
  - (c) emitter 形状评审 ✅（Cylinder 用于 Tornado / Lightning / DustDevil 三柱状；AABB 用于 AshFall / FogVeil / EmberDrift / HeatHaze / SnowDrift 五扁平区；Sphere 留 v2 给阵法力场，本 plan 不需要）
  - 下一步：进 P0，落 `server/src/world/environment.rs` + 双端 schema 镜像
- **2026-05-09**：完成 MC 现有龙卷风 mod 调研（subagent / Sonnet）。结论入骨架：
  - **Weather2 (Corosauce)** 1.20.1 是最直接借鉴对象，纯 CPU 粒子，零 shader，已在 1.12→1.20 验证；其 `TornadoFunnelSimple` 用 30-50 层环 × 每层 ~30 个大尺寸 cloud256 半透明 billboard ≈ 1500 粒子叠出"实心"
  - **风墙实心层路径调整**：从 demo 早期 "WallStreak ribbon 高速旋转" 改为 "cloud256 大 billboard 分层环"。单粒子从 16 quads（ribbon）降到 1 quad（sprite billboard），同视觉省 ~16x GPU。§1 表 `TornadoColumn` 实装路径已更新加注 Weather2 来源
  - **物理推力**：Weather2 `spinEntityv2` 三分量合速度（角度偏移 + 径向衰减 + Y 上升）作官方参考实现，详见 `plan-zone-weather-v1 §3`
  - **不适用方案**：ProtoManly raymarched shader（违反零光影约束、要求 GTX 1660+）
  - **新依赖锁定**：256px 灰云噪声贴图（`bong:cloud256_dust`）入仓需求，已加 §5 第 8 项开放问题；可走 `/gen-image particle` skill 立即生成首版 PNG

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：`server/src/world/environment.rs`（EnvironmentEffect enum + Registry + lifecycle event）+ `agent/packages/schema/src/zone-environment.ts`（双端 schema） + Rust 镜像
  - P1：`client/src/main/java/.../environment/`（Registry + 4 emitter 实装 + culling/fade）
  - P2：`MixinFogPerZone` / `MixinSkyPerZone` + audio loop 联动
  - P3：剩余 4 emitter + 三方接入示例（scorch / tribulation / tsy）+ 性能基线
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - server：`EnvironmentEffect` enum / `ZoneEnvironmentRegistry` / lifecycle event / broadcast system / physics hook trait
  - agent：`EnvironmentEffectV1` / `ZoneEnvironmentStateV1` schema 注册 + sample 对拍
  - client：`EnvironmentEffectRegistry` / `EmitterBehavior` / 8 emitter 实装 / 2 mixin
- 遗留 / 后续：
  - vanilla rain renderer 替换（§5 第 1 项，v2+）
  - Agent → server 注入 API（§5 第 5 项）
  - client-only ephemeral emitter（§5 第 6 项，归 plan-vfx 范围）
