# Bong · plan-perception-v1.1 · 视觉与神识感知系统

> 本版本**取代** `plan-perception-v1.md`（v1 仅模板态，未实施）。v1.1 主要变更：
> - **去 Iris**：删除 §2.3 Phase 2 与 §6 Phase 4 的 shader pack 依赖，全部走 vanilla 1.20.1 mixin + post-effect 实现；shader 真模糊路线另议，不阻塞本 plan
> - **基于 Minecraft chunk 加载约束重做视距表**：从 50/70/100/150/300/800m 现实化为 **30/50/80/120/180/240m**（化虚吃满主流 16 chunks render distance；高端玩家调到 32 chunks 时延伸到 vanilla 协议上限 512m）。fog 是压缩视野不是延伸——超出 render distance 是天空盒。Server `ViewDistance` 按境界动态绑（Valence default 仅 2 chunks，必须按境界推 chunks）
> - **三项开放问题决议**：(1) 视距首尾界点（醒灵≈凡人 / 化虚≈天人界）拟入 worldview §境界，中间四阶 plan 自治；(2) 化虚神识改为**三圈分层**（内 500m / 中 2000m / 外 ∞ event-bus 订阅）+ 5/20/50 tick 节流；(3) 隐匿 hook 升级为 `obfuscate_sense_kind`，按境界差 Δ 返回三态（Δ≥2 完全识破 / Δ=1 模糊化为 `AmbientLeyline` / Δ≤0 屏蔽），默认透传，留给 plan-stealth 替换
> - **补接入面 checklist**：按 `docs/CLAUDE.md §二` 列出进料/出料/共享类型/跨仓库契约/worldview 锚点
> - **每 Phase 加可 grep 抓手**：模块路径、类型名、schema 名、测试声明明确化，下游 `/plans-status` `/audit-plans-progress` 可机器核验
> - **新增 §10 测试矩阵**：所有视觉参数走纯函数 Planner，mixin 仅转发；headless 自动化覆盖 ≥ 200 用例，最后 1cm GL 胶水通过 RecordingSink 隔离测试

**视野与感知系统专项**。定义 Bong 的双感知系统：**肉眼视觉**（受境界限制的清晰距离 + fog + vignette + tint + 粒子层）与**神识感知**（worldview 已定义的境界世界感知能力，UI 形态采用屏幕边缘指示器）。

**核心理念**：
- **视觉受限是修仙感的基石** —— 醒灵境就是个普通人，远处全是雾
- **神识独立于视觉** —— 墙后/视觉外的感知，是境界提升的实质性奖励
- **两套系统并行**，不互相替代
- **纯 vanilla 1.20.1 实现**，不依赖 Iris/OptiFine 等 shader pack

**阶段总览**：

| Phase | 主题 | 状态 |
|-------|------|------|
| Phase 0 | 基础设施（schema、状态容器、mixin 骨架） | ⬜ |
| Phase 1 | 境界基础视距 L0 层 | ⬜ |
| Phase 2 | 神识边缘指示器 | ⬜ |
| Phase 3 | 状态/环境修正层（入定/入魔/负灵域） | ⬜ |

---

## §0 设计轴心

- **视觉 + 神识双系统并行**，不混淆
- **严格对齐 worldview §境界**：6 阶（醒灵/引气/凝脉/固元/通灵/化虚），神识能力直接抄 worldview 表
- **服务端权威**：视觉距离 / 神识范围由 server 决定并推送，client 不可改
- **沉浸式神识 UI**：屏幕边缘指示器（不是雷达圆盘，不是小地图）
- **负灵域机制对接**：高境界在负灵域视觉降档，呼应 worldview §负灵域
- **零 shader 依赖**：远距视觉一律走 vanilla fog + vignette + 粒子层 + 全屏 quad；shader 真模糊路线另议
- **逻辑/渲染分层**：所有视觉参数（fog 距离/RGB/alpha/tint）由纯函数 Planner 计算，Mixin 只做 `Planner 输出 → RenderSystem` 转发；mixin 内不放业务逻辑
- **视距上限受 client render distance 约束**：fog 是压缩视野不是延伸（超出 render distance 是天空盒）。server 按境界动态推送 chunks（`ViewDistance` 绑境界）；化虚境视距吃满主流 16 chunks render distance（256m），高端玩家自调 32 chunks 时延伸到 vanilla 协议上限 512m
- **命名空间消歧**：实装层禁用 "Perception" 词根（避开已有 `NarrationStyle::Perception` / `InsightCategory::Perception` / `VisualEffectProfile.PERCEPTION`）。server 用 `RealmVision` / `SpiritualSense`；client visual 子目录 `realm_vision/`

---

## §0.5 接入面 checklist

**进料**：
- `server/src/cultivation/components.rs::Realm`（6 阶 enum，已实装）
- `server/src/cultivation/breakthrough.rs::BreakthroughEvent`（突破成功 → 视距重算）
- `server/src/cultivation/death_hooks.rs::PlayerRevived`（境界跌落 → 视距收缩 + 平滑过渡）
- `server/src/zone/...`（负灵域标签，待 plan-tsy / plan-zone 对接 — 若 Phase 3 启动时仍未就位，先以 `NegativeZoneTag` 占位 mock）
- 客户端环境：vanilla `World.getDimension()` 时间 + `World.isRaining()` / `isThundering()`

**出料**：
- `bong:server_data` 新增 `ServerDataPayloadV1::RealmVisionParams { fog_start, fog_end, fog_color_rgb, fog_shape, vignette_alpha, tint_color_argb, particle_density, transition_ticks, server_view_distance_chunks, post_fx_sharpen }`
- `bong:server_data` 新增 `ServerDataPayloadV1::SpiritualSenseTargets { entries: Vec<SenseEntryV1>, generation }`，`SenseEntryV1 = { kind: SenseKindV1, x, y, z, intensity }`，`SenseKindV1 ∈ { LivingQi, AmbientLeyline, CultivatorRealm, HeavenlyGaze, CrisisPremonition }`
- 直接调整 Valence `ViewDistance` component（玩家境界变化时同步刷新）—— 这是反作弊关键，client 不能扩 fog 因为 chunks 根本没推过来

**共享类型**：
- 复用 `cultivation::components::Realm`（不新建近义 enum）
- 复用 `VisualEffectController` 的临时事件层（顿悟金光 `ENLIGHTENMENT_FLASH` 已实装，本 plan 不增）
- 新建 `RealmVisionState`（client 常驻状态机）独立于 `VisualEffectState`（生命周期/语义不同：前者常驻、后者衰减事件）

**跨仓库契约**：
- TS schema：`agent/packages/schema/src/realm-vision.ts`（TypeBox）+ `agent/packages/schema/src/spiritual-sense.ts`
- Sample fixtures：`agent/packages/schema/samples/realm-vision-{awaken,induce,condense,solidify,spirit,void}.sample.json` + `spiritual-sense-{induce,condense,solidify,spirit,void}.sample.json`
- Server：`server/src/cultivation/realm_vision/{params,planner,push,modifiers}.rs` + `server/src/cultivation/spiritual_sense/{scanner,throttle,push}.rs`
- Server schema：`server/src/schema/realm_vision.rs` 新增 `RealmVisionParamsV1` / `SpiritualSenseTargetsV1` + 对应 `ServerDataPayloadV1` variant
- Client：`com.bong.client.visual.realm_vision.{RealmVisionPlanner, RealmVisionState, RealmVisionStateReducer, RealmVisionCommand, RealmVisionInterpolator, FogParamsSink, GlFogParamsSink, RealmVisionTintRenderer, PerceptionEdgeProjector, PerceptionEdgeRenderer, PerceptionEdgeState}`
- Mixin：`MixinBackgroundRendererRealmVision`（仅做 Planner 输出 → `FogParamsSink` 转发）+ `MixinInGameHudRealmVignette`（vignette alpha 注入）

**worldview 锚点**：
- `worldview.md §境界`（神识能力表直接采用其"世界感知"列）
- `worldview.md §境界`（**待修订**：在境界表加首尾两端的"凡眼视距锚"——醒灵≈凡人 30m / 化虚≈天人 240m+；中间四阶 plan 自治）
- `worldview.md §负灵域`（视距压制是"高灵压抽吸"的视觉化第一表征）
- `worldview.md §602`（外观由境界段位决定，但视觉不精确分辨境界）
- `worldview.md §604`（看本质必须靠神识）
- `worldview.md §55`（负压战术：弱者诱强者入负灵域 → 视觉骤降 → 反杀）

---

## §1 与 worldview 对齐：神识感知逐阶定义

直接采用 worldview §境界 表的"世界感知"列：

| 阶 | 境界 (`Realm` enum) | 神识感知（worldview 已定义） | `SenseKindV1` |
|----|------|----------------------------|--------------|
| 0 | 醒灵 (`Awaken`) | 能看到灵气浓淡（HUD 灵气条） | — (HUD 现有) |
| 1 | 引气 (`Induce`) | 能感知 50 格内**生物气息**（含墙后） | `LivingQi` |
| 2 | 凝脉 (`Condense`) | 能感知**区域灵气精确值** | `AmbientLeyline` |
| 3 | 固元 (`Solidify`) | 能感知**其他修士的大致境界**（worldview §604） | `CultivatorRealm` |
| 4 | 通灵 (`Spirit`) | 能感知**天道注意力**（危机预警） | `HeavenlyGaze`、`CrisisPremonition` |
| 5 | 化虚 (`Void`) | 服务器内仅 1-2 人（神识近乎全图） | 全部 5 类叠加 |

**注意**：这不是视觉，是"内观/神识/灵感"层面的额外感知。墙后能感知，但不能"看见"。

---

## §2 视觉系统（worldview 未定义，本 plan 自治）

### 2.1 视觉距离表（基于 16 chunks 主流配置设计）

**设计锚点**：以 16 chunks (256m) 作为主流玩家 client render distance 设计基线，化虚境吃满锚点。Server `ViewDistance` 按境界动态调整，既是性能优化也是反作弊（低境界 client 改 fog 也看不到，因为 chunks 根本没推送）。

| 阶 | 境界 | 清晰视距 | 模糊起 | 全雾 | server `ViewDistance` (chunks) | fog 形状 | fog 色 RGB | vignette α | 屏幕 tint ARGB | 环境粒子层密度 |
|----|------|---------|--------|------|-------------------------------|---------|-----------|----------|--------------|-------------|
| 0 | 醒灵 | 30m | 30m | 60m | 4 (64m) | Cylinder | `0xB8B0A8` 暖灰 | 0.55 | `0x0FF0EDE8` 淡褐 | 0 |
| 1 | 引气 | 50m | 50m | 96m | 6 (96m) | Cylinder | `0xB0B5B8` 中灰 | 0.45 | `0x00000000` 透 | 0.05 |
| 2 | 凝脉 | 80m | 80m | 128m | 8 (128m) | Sphere | `0xA8B0BC` 蓝灰 | 0.35 | `0x0AE8F0FA` 极淡蓝 | 0.20 |
| 3 | 固元 | 120m | 120m | 192m | 12 (192m) | Sphere | `0x9CA8B8` 冷蓝灰 | 0.22 | `0x00000000` 透 | 0.45 |
| 4 | 通灵 | 180m | 180m | 256m | 16 (256m) | Sphere | `0x8898AA` 透蓝 | 0.10 | `0x00000000` 透 | 0.65 |
| 5 | 化虚 | 240m | 240m | 320m | 20 (320m, preview/手动调高 → 32 chunks 512m) | Sphere | `0x7888A0` 极淡蓝 | 0.0 | `0x05FFF8E8` 极淡金 | 0.85 |

**设计依据**：
- **醒灵 = 普通人**：30m 看清表情、60m 全雾。对应现实人眼可清晰辨认面部的距离 + Minecraft 4 chunks 加载范围
- **化虚 = 主流硬件天花板**：240m 视距吃满 16 chunks render distance（vanilla 默认 12 即 192m，升 16 主流流畅）；32 chunks 是 vanilla 协议绝对上限，preview/高端玩家自调可达 512m
- **视距梯度 30→50→80→120→180→240m**：每阶 1.4-2x 涨幅，醒灵 → 化虚 = 8x；突破时玩家有"我看得更远了"的实质感受，且每阶都在 client 能看到的 chunks 范围内
- **server `ViewDistance` 与 fog 距离对齐**：fog end ≤ `ViewDistance × 16 - 4m`（留余量避免 fog 边界硬切到天空盒）
- **fog 颜色暖→冷的色温迁移**：暖灰=凡尘烟火气重，蓝灰=灵气感渐起，淡蓝=接近"天道色"，对应修仙小说"出尘味"色调
- **fog 形状 Cylinder→Sphere 切换**：Cylinder 是地平线雾（更"地球感"），Sphere 是球形雾（更梦幻），凝脉起切 Sphere
- **vignette 0.55 → 0**：醒灵被框死在凡眼视野里，化虚"无界"
- **粒子密度 0 → 0.85**：低境界看不到灵气，凝脉起灵气可视化逐级丰富（worldview"灵气浓度可感知"的视觉化）
- **化虚视距优势削弱由"氛围卷"补偿**：见 §5.4 化虚境视觉补偿

### 2.2 视觉环境调整因子

视觉距离不是只看境界，还有：

| 因子 | 视距乘数 | 额外效果 |
|------|---------|---------|
| 夜间 | × 0.3 | 月相再调（满月 × 0.5） |
| 暴雨/雪 | × 0.5 | 跟 vanilla weather 同步 |
| 普通雾区 | × 0.4 | 由 worldgen 标记 |
| **负灵域** | **× max(0.5, 1 − 0.15 × realm_idx)** | **化虚境进负灵域 ≈ × 0.125**，从 240m 骤降到约 30m；进入瞬间用 0.4s 快速插值（撕裂感）+ 边缘灰白闪现 200ms |
| 灵脉富集区 | × 1.2 | 灵气浓度高反而看得清 |
| 入定状态 | × 1.5 | 屏幕淡青 + vignette × 0.5 |
| 入魔状态 | × 0.5 | 边缘深红黑（复用 `DEMONIC_FOG` profile） |
| 顿悟瞬间（3-5s） | × 3.0 | 全屏金白闪（复用 `ENLIGHTENMENT_FLASH`）→ 之后 8s 内插值回基线 |
| 天劫接近 | × 0.7 | 整屏灰蓝压抑 |
| 中毒/寒毒 | × 0.8 | 对应色调染色 |

**叠加规则**：

```
// 1. 算境界基线
base_clear = base_clear_table(realm)               // 30/50/80/120/180/240
base_view  = base_view_distance_chunks(realm)      // 4/6/8/12/16/20

// 2. 状态 + 环境因子按乘法链叠加
modified_clear = base_clear × ∏(env_modifiers) × ∏(status_modifiers)

// 3. 受 server view distance 上限约束（fog 不能超出 chunks 加载范围）
final_clear = min(modified_clear, base_view × 16 - 4)

// 4. 下限 clamp（不至于瞎到看不见脚下）
final_clear = max(final_clear, FLOOR_CLAMP_M)      // FLOOR_CLAMP_M = 15m

// 5. fog end 对齐 chunks 加载边界
final_fog_end = min(final_clear × end_to_clear_ratio(realm), base_view × 16 - 4)
```

`FLOOR_CLAMP_M = 15m`：醒灵 30m × 0.3 (夜) × 0.5 (雨) = 4.5m 不合理，clamp 到 15m 保证最差情况下还能看到脚下一圈。

**负灵域设计要点**：化虚境进入负灵域可能视觉降到醒灵水平甚至更低，呼应 worldview"高境界被抽干"的机制——视觉是被抽干的第一表征。注意 `negative_zone` 公式 `max(0.5, 1 - 0.15 × realm_idx)` 醒灵 idx=0 → 1.0 不压制，化虚 idx=5 → 0.25（叠加 × 0.5 基数 = × 0.125），符合 worldview "高境界被抽干"的非线性。

### 2.3 实现路径（纯 vanilla 1.20.1）

**Server 端**：
- **Valence `ViewDistance` component 按境界绑**：参考 `server/src/preview/mod.rs::boost_view_distance_for_preview` 的写法，在 `BreakthroughEvent` / `PlayerRevived` 系统里直接 `view_distance.set(realm_to_chunks(realm))`
- **chunks 推送平滑过渡**：突破时 `ViewDistance` 不一次性扩到目标值，而是每秒扩 2 chunks（5-10s 内逐步扩），避免 N 个 chunks 同时推送造成网络抖动；跌落时同理逐步收缩

**Client 端 vanilla mixin**：
- **Mixin `BackgroundRenderer.applyFog`**：`@Inject(method = "applyFog", at = @At("TAIL"))`，调用 `RenderSystem.setShaderFogStart/End/Color/Shape`，参数从 `RealmVisionPlanner.plan(...)` 输出取
- **Mixin `InGameHud.renderVignetteOverlay`**：注入 vignette alpha
- **整屏 tint**：复用 `OverlayQuadRenderer` 画一层全屏半透 quad
- **环境粒子层**：vanilla `ParticleManager` 持续 spawn（按密度参数 + 朝向算频率）；新建 `LeylineParticleType`（自定义 ParticleType `bong:leyline_drift`）
- **境界跌落淡入**：client 端 5-10s 数值插值（lerp fog params），由 `RealmVisionInterpolator` 纯函数计算

**Client render distance 引导**（无法强制）：
- `MinecraftClient.options.viewDistance` 是用户画质设置，mod 不可强制覆盖
- 客户端首次进入服务器 → 检测 `viewDistance < 12` → toast 提示：「**修仙世界推荐 render distance ≥ 16 chunks，否则视野体验受限**」
- 不挡用户进游戏，仅引导

**shader 真模糊路线另议**：vanilla post-effect pipeline (`assets/minecraft/shaders/post/*.json`) 和 Iris shader pack 都能做更精细的 depth-aware 模糊，但本 plan 不依赖、不实施。如未来另起 plan 加入，将以叠加层方式接入，不破坏本 plan 的 vanilla 基线。

---

## §3 神识系统：屏幕边缘指示器

### 3.1 形态规范

参考《死亡搁浅》的 BB 探测器、《地平线》的专注模式：

```
       ↑                    
   ╔═══════╗
   ║       ║  ←─ 屏幕边缘
   ║       ║
←  ║       ║  →  ← 视野外的感知目标 = 边缘指示器
   ║       ║
   ║       ║
   ╚═══════╝
       ↓
```

- 视野**外**的感知目标在屏幕**对应方向边缘**显示一个微光标记
- 视野**内**的目标不显示边缘指示器（避免遮挡）
- 距离越近指示器越大/越亮
- 不同感知类型用不同颜色/形状

### 3.2 指示器类型与境界对应

| 触发境界 | `SenseKindV1` | 视觉 | 含义 |
|---------|--------|------|------|
| 1 引气 | `LivingQi` | 淡白色圆点 | 50m 内活物（人/兽） |
| 2 凝脉 | `AmbientLeyline` | 蓝色波纹 | 区域灵气富集点 |
| 3 固元 | `CultivatorRealm` | 数字/段位符号 | 其他修士的境界段位 |
| 4 通灵 | `HeavenlyGaze` | 红色裂纹 | 天道注意力聚焦的方向 |
| 4 通灵 | `CrisisPremonition` | 闪烁红圈 | 即将发生的负面事件方向 |
| 5 化虚 | 全 5 类叠加 | 多重叠加 | 几乎全图 |

### 3.3 不做什么（明确边界）

- ❌ **不做小地图/雷达圆盘**——破坏沉浸感
- ❌ **不在 3D 世界中标点**（不做 Highlight Outline / 透视标记）——保留视觉的纯粹
- ❌ **不显示具体名字/血量**——神识是"感"不是"读取"
- ❌ **不替代视觉**——视野内的目标必须靠肉眼看，神识只补充视野外

### 3.4 实现路径

- 客户端 `PerceptionEdgeRenderer`（接 `BongHudOrchestrator`）
- `PerceptionEdgeState` 容器：从 `bong:server_data SpiritualSenseTargets` 读取目标列表（位置 + 类型 + 强度）
- `PerceptionEdgeProjector`（**纯函数**）：输入 (相机位置、相机方向、屏幕宽高、目标世界坐标) → 输出 `EdgeIndicatorCmd { x, y, kind, intensity, on_edge: bool }`
  - 视野内（投影在屏幕内）→ `on_edge=false`，不画
  - 视野外 → 投影到屏幕边缘最近交点
  - 摄像机背后 → 投影到屏幕边缘对侧
- 优先级 + 数量上限：每方向最多 3 个指示器，按 intensity 排序

---

## §4 服务端权威与同步

### 4.1 视觉距离

- server 根据玩家境界 + 环境因子计算 fog 距离（`server/src/cultivation/realm_vision/planner.rs::compute_vision_params(...)` 纯函数）
- 通过 `bong:server_data` 推送 `RealmVisionParams { fog_start, fog_end, fog_color_rgb, fog_shape, vignette_alpha, tint_color_argb, particle_density, transition_ticks, server_view_distance_chunks, post_fx_sharpen }`
- **同步调整 Valence `ViewDistance`**：每次推 RealmVisionParams 时，server 端同时 `view_distance.set(target_chunks)`；突破/跌落场景下用 ramp system 每秒扩/收 2 chunks，5-10s 内逐步过渡（避免一次性推大量 chunks 卡网络）
- 推送时机：境界变化（`BreakthroughEvent` / `PlayerRevived`）、环境因子变化（进出负灵域 / 天气切换 / 昼夜切换）、状态切换（入定/入魔/中毒）
- client 收到后调 `RealmVisionStateReducer.apply(...)`，Planner 每帧产出 `RealmVisionCommand`，Mixin 通过 `FogParamsSink` 喂 `RenderSystem`
- **反作弊**：因为低境界玩家 server 只推 4-12 chunks，client 即使改 mod 扩 fog 距离，远处也是空气/天空盒，看不到任何东西

### 4.2 神识感知列表

**通用境界（引气/凝脉/固元/通灵）**：
- server 每 5 tick 按玩家境界扫描感知半径内目标（`server/src/cultivation/spiritual_sense/scanner.rs`）
- 推送 `SpiritualSenseTargets { entries, generation }`
- 半径表：引气 50m（worldview）/ 凝脉 200m / 固元 500m / 通灵 1000m
- 空间索引：chunk-based grid（每 chunk 维护轻量实体引用列表），Phase 0 先 `O(n)` 扫描，Phase 2 视性能换 grid

**化虚境（三圈分层）**：

| 圈 | 半径 | 扫描内容 | 节流 | 实现 |
|----|------|---------|------|------|
| 内圈 | 500m | 所有目标（同通灵境精度） | 5 tick | grid 扫描 |
| 中圈 | 2000m | 仅修士 / 战斗 / 灵气波动 | 20 tick | grid 扫描 + 类型过滤 |
| 外圈 | ∞ (整服) | 仅**大事**：天劫 / 灾劫 / 其他化虚境 / 时代法旨 | 50 tick (~2.5s) | **订阅 server event bus**（`BreakthroughEvent` / `TribulationFailed` / `EraDecree` / 化虚境广播），按类型过滤；不做空间扫描 |

**化虚分层设计意图**：worldview 说化虚"神识近乎全图"——但不是看到每只蚂蚁，是"近处明察、远处感大势"。外圈 ∞ 是 event-bus 订阅+类型过滤，没有 O(N²) 性能问题；payload 也小（大事类事件天然稀疏）。

### 4.3 反作弊

- client 不可自行扩大 fog 距离（即使改设置文件）
- 因为 Mixin 直接覆写 vanilla fog 参数，且 server `ViewDistance` 也按境界绑住，远处 chunks 根本没推过来
- 客户端"渲染距离"画质设置仍可调（用户体验上限），但 fog 由 server 强制覆盖，且 chunks 加载范围由 server 决定

### 4.4 隐匿与神识

worldview 提到"隐匿"机制，需要 plan-stealth 配合。本 plan 留 hook，按境界差驱动三态结果：

```rust
// server/src/cultivation/spiritual_sense/scanner.rs
pub fn obfuscate_sense_kind(
    original_kind: SenseKindV1,
    observer_realm: Realm,
    target_realm: Realm,
    target_stealth: Option<&StealthState>,  // plan-stealth 提供
) -> Option<SenseKindV1> {
    // 默认实现（无 stealth）：透传
    if target_stealth.is_none() {
        return Some(original_kind);
    }
    // 由 plan-stealth 覆盖此分支：
    //   delta = observer_realm.idx() as i8 - target_realm.idx() as i8
    //   delta >= 2  -> Some(original_kind)        // 完全识破
    //   delta == 1  -> Some(SenseKindV1::AmbientLeyline)  // 模糊化为灵气波动
    //   delta <= 0  -> None                       // 完全屏蔽
    Some(original_kind)
}
```

**境界差三态设计意图**：
- **Δ ≥ 2 完全识破**：高境界天然识破低境界隐匿（不公平也不应该公平）
- **Δ = 1 模糊化**：平级博弈，隐匿者掌握"被警觉但不被认出"的窗口
- **Δ ≤ 0 完全屏蔽**：低对高隐匿稳，呼应 worldview §55 负压战术（弱者诱强者）

本 plan 实施默认透传逻辑 + 接口签名；境界差三态判断由 plan-stealth 立项时实施。"接口先于实现锁定，测试同时锁定接口"——本 plan 的测试覆盖默认透传 + 三种返回结果的 schema 兼容性。

---

## §5 与其他系统的协调

### 5.1 与 HUD 叠色（plan-vfx-v1 §3）

| 状态 | 视觉影响 | 叠色（已实装的 VisualEffectProfile） |
|------|---------|----------|
| 入定 | 视觉 × 1.5 | 整屏淡青（待新增 profile） |
| 入魔 | 视觉 × 0.5 | 边缘黑雾（`DEMONIC_FOG` 已实装） |
| 顿悟瞬间 | 视觉 × 3（数秒）| 全屏金光闪（`ENLIGHTENMENT_FLASH` 已实装） |
| 天劫接近 | 视觉 × 0.7 | 整屏灰蓝压抑（待新增 profile） |
| 中毒/寒毒 | 视觉 × 0.8 | 对应色调染色（待新增 profile） |

视距数值修正由本 plan 的 server 端 modifier chain 计算；屏幕叠色复用 `VisualEffectController` 的临时事件层 → 多重叠加。

### 5.2 与战斗系统

- 低境界打高境界："对方在我视野外但我在对方视野内" → 自然劣势
- 弱者诱强者入负灵域：高境界视觉骤降到醒灵水平，盲战 → 反杀机会（worldview §55"负压战术"的视觉化实现）
- 神识 + 隐匿：worldview 提到"隐匿"机制，需要 plan-stealth 配合（隐匿玩家不出现在神识列表，见 §4.4）

### 5.3 与 worldview §602"外观由境界段位决定"

- 视觉看到的"人形外观"由对方境界段决定（醒灵到化虚穿着不同）
- **但视觉不精确分辨境界**（引气和凝脉看起来差不多 —— worldview 原话）
- **要看本质必须靠神识**（固元境的"看到对方大致境界段"是神识能力 `CultivatorRealm`，不是视觉）

### 5.4 化虚境视觉补偿（关键：现实视距下的天人感）

由于 vanilla 协议视距上限约束，化虚境视距优势从原方案 800m 削弱到 240m（仅比通灵 180m 多 60m），单看视距已经体现不出"天人之界"。补偿手段：

| 化虚境额外视觉效果（不需要更远视距） | 实现 |
|-------------------------------------|------|
| **粒子层最高密度（0.85）**：看到"灵气河流"3D 流动 | vanilla `ParticleManager` |
| **整屏极淡金辉 tint（`0x05FFF8E8` 低 alpha）**：超验感 | `OverlayQuadRenderer` |
| **vignette = 0**：完全无边界感 | `MixinInGameHudRealmVignette` |
| **远端反锐化（`post_fx_sharpen` 字段）**：远处地形细节更亮（near→far 不糊反清） | vanilla post-effect 近似（`assets/minecraft/shaders/post/realm_void_sharpen.json` 自定义） |
| **神识 = 真正的全图感知**（外圈 ∞ event-bus 订阅大事） | §4.2 三圈分层 |

**核心理念**：**化虚的"看更远"用神识体验，不是用肉眼。**这反而更符合 worldview——化虚是"超脱凡眼"的境界，凡眼极限就停在 240m，更远靠"散神识感大势"。

---

## §6 实施节点

### Phase 0：基础设施 ⬜

**Schema**：
- [ ] `agent/packages/schema/src/realm-vision.ts`：TypeBox 定义 `RealmVisionParamsV1`（含 `server_view_distance_chunks: u8` 与 `post_fx_sharpen: f32` 字段）+ `FogShapeV1` enum (`Cylinder` / `Sphere`)
- [ ] `agent/packages/schema/src/spiritual-sense.ts`：TypeBox 定义 `SpiritualSenseTargetsV1` + `SenseKindV1` + `SenseEntryV1`
- [ ] `agent/packages/schema/samples/realm-vision-{awaken,induce,condense,solidify,spirit,void}.sample.json` 6 份（含 server_view_distance_chunks: 4/6/8/12/16/20）
- [ ] `agent/packages/schema/samples/spiritual-sense-{induce,condense,solidify,spirit,void}.sample.json` 5 份
- [ ] `server/src/schema/realm_vision.rs`：serde struct `RealmVisionParamsV1` / `SpiritualSenseTargetsV1` + `FogShapeV1` enum，sample roundtrip 测试
- [ ] `server/src/schema/server_data.rs::ServerDataPayloadV1` 新增 `RealmVisionParams(...)` 与 `SpiritualSenseTargets(...)` variant + `ServerDataPayloadWireV1` 对应 wire form

**Server 骨架**：
- [ ] `server/src/cultivation/realm_vision/mod.rs` 模块挂载
- [ ] `server/src/cultivation/realm_vision/params.rs::RealmVisionParams` struct（与 schema 镜像）
- [ ] `server/src/cultivation/spiritual_sense/mod.rs` 模块挂载

**Client 骨架**：
- [ ] `com.bong.client.visual.realm_vision.RealmVisionState`（client 状态容器）
- [ ] `com.bong.client.visual.realm_vision.RealmVisionStateReducer`（payload → state，纯函数）
- [ ] `com.bong.client.visual.realm_vision.PerceptionEdgeState`
- [ ] `client/...network/BongNetworkHandler` 增加新 variant 分发

**测试**：
- [ ] schema TypeBox validate 11 个 sample
- [ ] server `cultivation::realm_vision::tests::payload_serialize_roundtrip` 6 阶
- [ ] server `cultivation::realm_vision::tests::server_data_v1_realm_vision_variant` 1
- [ ] client `RealmVisionStateReducerTest` ≥ 8 case
- [ ] client `BongNetworkHandlerPayloadFixtureTest` 增加 realm-vision/spiritual-sense fixture（5+ case）

---

### Phase 1：境界基础视距（L0 层）⬜

**Server**：
- [ ] `server/src/cultivation/realm_vision/planner.rs::compute_base_params(realm: Realm) -> RealmVisionParams` 纯函数，6 阶视距表（30/50/80/120/180/240m + view_distance 4/6/8/12/16/20 chunks）内嵌
- [ ] `server/src/cultivation/realm_vision/push.rs`：监听 `BreakthroughEvent` + `PlayerRevived` → 计算并推送
- [ ] `server/src/cultivation/realm_vision/view_distance_ramp.rs`：`ViewDistanceRampSystem`，每秒扩/收 ≤2 chunks 平滑过渡
- [ ] 突破/跌落时直接调整 Valence `ViewDistance` component（参考 `server/src/preview/mod.rs::boost_view_distance_for_preview` 写法）
- [ ] 注册 system 到 `cultivation/mod.rs` 的执行顺序

**Client**：
- [ ] `com.bong.client.visual.realm_vision.RealmVisionPlanner.plan(state, tick) -> RealmVisionCommand` 纯函数
- [ ] `com.bong.client.visual.realm_vision.RealmVisionCommand` record（`fogStart, fogEnd, fogColorRgb, fogShape, vignetteAlpha, tintColorArgb, particleDensity, postFxSharpen`）
- [ ] `com.bong.client.visual.realm_vision.FogParamsSink` interface + `GlFogParamsSink` production impl（≤ 5 行纯转发）
- [ ] `client/...mixin/MixinBackgroundRendererRealmVision.java` `@Inject(applyFog, TAIL)` 调 `FogParamsSink`
- [ ] `client/...mixin/MixinInGameHudRealmVignette.java` 注入 vignette alpha
- [ ] `com.bong.client.visual.realm_vision.RealmVisionTintRenderer`：复用 `OverlayQuadRenderer` 画整屏 tint
- [ ] `com.bong.client.visual.realm_vision.ClientRenderDistanceAdvisor`：检测 `MinecraftClient.options.viewDistance < 12` → toast 提示推荐 ≥16
- [ ] `assets/bong/particles/leyline_drift.json` 自定义粒子定义 + `com.bong.client.visual.realm_vision.LeylineParticle`

**测试**：
- [ ] server `realm_vision::tests::base_clear_distance_per_realm` 6 case（30/50/80/120/180/240m）
- [ ] server `realm_vision::tests::view_distance_per_realm` 6 case（4/6/8/12/16/20 chunks）
- [ ] server `realm_vision::tests::push_on_breakthrough` 6 case
- [ ] server `realm_vision::tests::push_on_revive` 6 case
- [ ] server `realm_vision::tests::view_distance_ramp_smoothing` ≥ 4 case（突破/跌落每秒 ≤2 chunks 过渡）
- [ ] server `realm_vision::tests::final_clear_clamp_to_view_distance` ≥ 4 case（fog end 不超 chunks 加载边界）
- [ ] client `RealmVisionPlannerTest::base_per_realm` 6 case
- [ ] client `RealmVisionPlannerTest::clamp_to_render_distance` ≥ 4 case
- [ ] client `MixinBgRendererSinkTest` ≥ 6 case（RecordingSink 断言 fog 参数序列）
- [ ] client `RealmVisionFixtureTest` 6 阶端到端 fixture 跑通
- [ ] client `ClientRenderDistanceAdvisorTest` ≥ 2 case（< 12 触发 toast / ≥ 16 不触发）

**端到端 demo（Phase 1 验收）**：
- 服务端：`/setrealm awaken` 命令将测试玩家境界设置为 `Awaken`
- 客户端可见：5s 内插值，视距从默认 → 30m，fog 颜色变暖灰，vignette 加重；server `ViewDistance` 同步降到 4 chunks
- `/setrealm void` → 5-10s 内 chunks 平滑扩到 20，视距扩到 240m（client render distance ≥16 时），fog 变极淡蓝，vignette 几乎消失，灵气粒子变浓

---

### Phase 2：神识边缘指示器 ⬜

**Server**：
- [ ] `server/src/cultivation/spiritual_sense/scanner.rs::scan_targets_inner_ring(observer, realm, world) -> Vec<SenseEntryV1>` 纯函数（通用境界 + 化虚内圈 500m）
- [ ] `server/src/cultivation/spiritual_sense/scanner.rs::scan_targets_mid_ring_void(observer, world) -> Vec<SenseEntryV1>` 纯函数（化虚中圈 2000m，仅修士/战斗/灵气波动）
- [ ] `server/src/cultivation/spiritual_sense/event_bus_subscriber.rs`：化虚外圈订阅 `BreakthroughEvent` / `TribulationFailed` / `EraDecree` / 化虚境广播 → 转 `SenseEntryV1`
- [ ] `server/src/cultivation/spiritual_sense/throttle.rs`：5 / 20 / 50 tick 三档节流（内/中/外圈）
- [ ] `server/src/cultivation/spiritual_sense/push.rs`：节流后推送 `SpiritualSenseTargets`
- [ ] `obfuscate_sense_kind` hook（默认透传，给 plan-stealth 留接口；签名见 §4.4）

**Client**：
- [ ] `com.bong.client.visual.realm_vision.PerceptionEdgeProjector.project(camera, screen, target) -> EdgeIndicatorCmd` 纯函数
- [ ] `com.bong.client.visual.realm_vision.PerceptionEdgeRenderer`（接 `BongHudOrchestrator`）
- [ ] 5 种指示器渲染（`LivingQi` 淡白圆点 / `AmbientLeyline` 蓝色波纹 / `CultivatorRealm` 段位符号 / `HeavenlyGaze` 红色裂纹 / `CrisisPremonition` 闪烁红圈）
- [ ] 优先级 + 数量上限（每方向 ≤ 3）

**测试**：
- [ ] server `spiritual_sense::tests::scan_targets_per_realm` 6 case（每境界扫描半径 + 目标分类）
- [ ] server `spiritual_sense::tests::three_ring_scan_for_void` 3 case（化虚内/中/外圈各扫到的目标类型分别正确）
- [ ] server `spiritual_sense::tests::throttle_intervals` 3 case（5/20/50 tick 节流）
- [ ] server `spiritual_sense::tests::obfuscate_hook_default_passthrough` 1 case（默认透传）
- [ ] server `spiritual_sense::tests::obfuscate_hook_three_state_schema` 3 case（mock plan-stealth 注入，覆盖 Δ≥2 / Δ=1 / Δ≤0 三种返回值的 schema 兼容性）
- [ ] client `PerceptionEdgeProjectorTest` ≥ 12 case（视野内 / 四边 / 四角 / 摄像机背后 / 优先级溢出）
- [ ] client `PerceptionEdgeRendererTest` ≥ 8 case（5 种指示器各一 + 优先级 + 数量上限 + 强度衰减）

**端到端 demo（Phase 2 验收）**：
- 凝脉境玩家面对一堵墙，墙后 30m 站另一玩家
- 屏幕墙的方向边缘出现淡白圆点（`LivingQi`），墙的方向也出现蓝色波纹（`AmbientLeyline`）
- 玩家转身 → 圆点 / 波纹消失（投影到背后边缘 → 视野外屏幕反方向出现）
- `/setrealm void` 后，触发另一区域的 `BreakthroughEvent` → 化虚境玩家屏幕边缘对应方向出现淡白圆点（外圈 event-bus 订阅生效）

---

### Phase 3：状态/环境修正层 ⬜

**Server**：
- [ ] `server/src/cultivation/realm_vision/modifiers.rs::apply_status_modifiers(params, status) -> params`
- [ ] `server/src/cultivation/realm_vision/modifiers.rs::apply_env_modifiers(params, env) -> params`
- [ ] 负灵域按境界递减压制公式：`max(0.5, 1 - 0.15 × realm_idx)`
- [ ] `FLOOR_CLAMP_M = 15m` 下限
- [ ] `transition_ticks` 字段写入 payload（境界跌落 5-10s = 100-200 ticks）
- [ ] 状态切换（入定/入魔/中毒）→ 推送修正后的 params

**Client**：
- [ ] `com.bong.client.visual.realm_vision.RealmVisionInterpolator.interpolate(from, to, ticks_remaining) -> RealmVisionParams` 纯函数
- [ ] Reducer 收到带 `transition_ticks > 0` 的 payload 时启动插值
- [ ] 负灵域进入瞬间：边缘灰白闪现 200ms（`OverlayQuadRenderer` 短脉冲）+ vignette 弹性收缩

**测试**：
- [ ] server `realm_vision::tests::status_modifier_chain` ≥ 20 case（入定/入魔/顿悟/天劫/中毒 单+组合）
- [ ] server `realm_vision::tests::env_modifier_chain` ≥ 30 case（夜×雨×雾×负灵域×灵脉 × 6 阶）
- [ ] server `realm_vision::tests::negative_zone_realm_scaling` 6 case
- [ ] server `realm_vision::tests::floor_clamp` ≥ 4 case
- [ ] server `realm_vision::tests::transition_ticks_payload` ≥ 8 case
- [ ] client `RealmVisionInterpolatorTest` ≥ 8 case（起 → 终 + tick → 中间帧）
- [ ] client `RealmVisionPlannerTest::with_status/env_modifier` ≥ 25 case

**端到端 demo（Phase 3 验收）**：
- 化虚境玩家走进负灵域边界 → 0.4s 内视距骤降（240m → ~30m）+ 边缘灰白闪 + vignette 弹性收缩
- 离开负灵域 → 3s 慢恢复
- `/setrealm spirit` 后立即 `/setrealm condense`（模拟跌落）→ 5-10s 内视距 + chunks 平滑收缩

---

## §7 已知风险

- **服务端 push 通道未铺**：Phase 0 是所有 Phase 的前置（已通过 `ServerDataPayloadV1` 落地，仅需扩展 variant）
- **vanilla 协议视距硬上限 32 chunks (512m)**：化虚境视距理论最大 240m × 8/16 chunks 自动延伸到 ~480m（高端配置），不能再高；化虚的"天人之感"靠氛围 + 神识 + post-fx 锐化补足（见 §5.4）
- **client `viewDistance` 不可强制**：是 vanilla 用户画质设置，mod 不能覆盖；只能用 `ClientRenderDistanceAdvisor` toast 提示推荐 ≥16，玩家配置低于 12 时视距体验受限
- **chunks 推送平滑过渡是硬要求**：突破 + 跌落场景下 server `ViewDistance` 一次性大幅变化会触发批量 chunks 推送，造成网络抖动；必须由 `ViewDistanceRampSystem` 每秒 ≤2 chunks 平滑过渡（5-10s 内完成）
- **边缘指示器视觉污染**：感知目标过多时屏幕边缘会被指示器塞满，需要**优先级 + 数量上限**（默认每方向最多 3 个，§3.4 已规划）
- **神识/视觉混淆**：玩家可能觉得"为什么我看到了又看不到" → 需要**入门引导**或 inspect 界面解释
- **化虚境扫描性能**：内圈 grid 扫描 + 中圈 grid + 类型过滤 + 外圈 event-bus 订阅，分层节流后压力可控；最大风险点是中圈 2000m 的 grid 扫描（覆盖区块多），需要在 Phase 2 末做 benchmark 测试
- **境界跌落的视觉冲击**：从固元降回凝脉，视觉骤缩 + chunks 收回 → 玩家可能不适，需要平滑过渡（5-10 秒淡入 + 渐收 chunks）
- **vanilla post-effect 局限**：Mojang 1.20.1 的 post pipeline 不暴露 depth buffer 给 fragment shader，无法做精确 depth-aware blur；本 plan 不依赖此能力，远距视觉效果靠 fog 渐隐 + 粒子层 + tint + 简单 sharpen 四件套近似
- **Mixin 与其他 mod 的兼容**：未来若引入 OptiFine 之类自改 fog 的 mod，可能冲突；当前 mod 列表干净，不阻塞

---

## §8 开放问题

**已决议**（v1.1 落地）：
- ✅ **视距表归属**：worldview §境界 加首尾两端的"凡眼视距锚"（醒灵≈30m 凡人 / 化虚≈240m+ 天人），中间四阶 plan 内自治。worldview 修订是另起 commit，不在本 plan 自动化范围
- ✅ **化虚神识半径**：三圈分层（内 500m grid 扫描 / 中 2000m grid + 类型过滤 / 外 ∞ event-bus 订阅），见 §4.2
- ✅ **隐匿机制对神识**：境界差三态（Δ≥2 完全识破 / Δ=1 模糊化为 `AmbientLeyline` / Δ≤0 屏蔽），hook 接口本 plan 留好，三态判断由 plan-stealth 立项时实施，见 §4.4

**仍待决议**：
- [ ] 神识"危机预警"（通灵境）的 UI 表现：边缘红圈 vs 屏幕震颤 vs HUD 警告？
- [ ] 是否给玩家"主动放出神识"的按键（按住 V 键临时增强感知）？还是永远被动？
- [ ] 多个相同方向的目标合并显示？还是叠加多个指示器？（默认 v1.1 §3.4 取每方向 ≤3 限制，但合并显示是更优雅的方向）
- [ ] inspect 界面里要不要可视化"我现在的感知范围"（一个圆圈在世界地图上）？

---

## §9 参考

**世界观对齐**：
- `docs/worldview.md §境界`（直接采用其"世界感知"列）
- `docs/worldview.md §负灵域`（视觉降档机制）
- `docs/worldview.md §602/604`（视觉与神识的边界）

**UI 灵感**：
- 《死亡搁浅》BB 探测器（边缘脉冲）
- 《地平线》专注模式（环境标记）
- 《看门狗》ctOS 视角（叠加 HUD 标记，本 plan 不采用此过强方案）

**前置 plan**：
- `plan-vfx-v1.md`（HUD 叠色协调，已落地）
- `plan-particle-system-v1.md`（服务端 VFX 事件 schema 与范围过滤前置）
- `plan-cultivation-v1.md`（境界系统数据源，已落地）

**未来探索（不阻塞本 plan）**：
- shader pack 真模糊：vanilla post-effect 或 Iris 集成路线另议，本 plan 不依赖

---

## §10 测试矩阵（防 agent 看不见的痛点）

**核心设计**：所有视觉效果先经过纯函数 Planner，Mixin 仅做"Planner 输出 → RenderSystem"的转发。任何视觉参数（fog 距离、RGB、vignette alpha、tint）都数值化，数值正确视觉就正确。

```
Server:  compute_vision_params(realm, status, env) → RealmVisionParams (pure)
              ↓ ServerDataPayloadV1::RealmVisionParams
Client:  RealmVisionStateReducer.apply(state, payload) → newState (pure)
              ↓
         RealmVisionPlanner.plan(state, tick) → RealmVisionCommand (pure)
              ↓
         FogParamsSink interface ←─── 测试用 RecordingSink 断言调用序列
              ↓ (production: GlFogParamsSink ≤ 5 行纯转发)
         RenderSystem.setShaderFog{Start,End,Color,Shape}  ← 唯一不可测的最后 1cm
```

### 10.1 Server (cargo test)

| 测试名 | 用例数 | 抓手 |
|------|--------|------|
| `realm_vision::tests::base_clear_distance_per_realm` | 6 | 6 阶基础值断言（30/50/80/120/180/240m） |
| `realm_vision::tests::view_distance_per_realm` | 6 | server `ViewDistance` 6 阶值断言（4/6/8/12/16/20 chunks） |
| `realm_vision::tests::view_distance_ramp_smoothing` | ≥ 4 | 突破/跌落每秒 ≤2 chunks 过渡断言 |
| `realm_vision::tests::final_clear_clamp_to_view_distance` | ≥ 4 | fog end ≤ chunks × 16 - 4，不超加载边界 |
| `realm_vision::tests::status_modifier_chain` | ≥ 20 | 入定/入魔/顿悟/天劫/中毒单+组合 |
| `realm_vision::tests::env_modifier_chain` | ≥ 30 | 夜×雨×雾×负灵域×灵脉 × 6 阶 |
| `realm_vision::tests::negative_zone_realm_scaling` | 6 | 负灵域按境界递减公式 |
| `realm_vision::tests::floor_clamp` | ≥ 4 | 醒灵 + 全 debuff ≥ 15m 下限 |
| `realm_vision::tests::transition_ticks_payload` | ≥ 8 | 境界跌落 transition_ticks 字段 |
| `realm_vision::tests::push_on_breakthrough` | 6 | BreakthroughEvent → 推送 |
| `realm_vision::tests::push_on_revive` | 6 | PlayerRevived → 推送 |
| `realm_vision::tests::payload_serialize_roundtrip` | 6 | RealmVisionParams serde 6 阶 sample（含 view_distance 字段） |
| `realm_vision::tests::server_data_v1_realm_vision_variant` | 1 | ServerDataPayloadV1 含新 variant 反序列化 |
| `spiritual_sense::tests::scan_targets_per_realm` | 6 | 每境界扫描半径 + 目标分类 |
| `spiritual_sense::tests::three_ring_scan_for_void` | 3 | 化虚内/中/外圈各扫到的目标类型 |
| `spiritual_sense::tests::throttle_intervals` | 3 | 5/20/50 tick 三档节流 |
| `spiritual_sense::tests::obfuscate_hook_default_passthrough` | 1 | 默认透传 |
| `spiritual_sense::tests::obfuscate_hook_three_state_schema` | 3 | mock plan-stealth 注入，覆盖 Δ≥2 / Δ=1 / Δ≤0 三态返回值 schema |
| `spiritual_sense::tests::server_data_v1_targets_variant` | 1 | ServerDataPayloadV1 含新 variant 反序列化 |

**总计 ≥ 124 用例**。

### 10.2 Schema (npm test in agent/packages/schema)

| 测试名 | 用例数 | 抓手 |
|------|--------|------|
| `typebox_validate_realm_vision` | 6 | 6 阶 sample × validate |
| `typebox_validate_spiritual_sense` | 5 | 各境界 sample × validate |
| `cross_lang_sample_match` | 11 | 跨端 fixture 对拍（rust serde + ts typebox 加载同一 JSON） |

**总计 ≥ 22 用例**。Server 端 `realm_vision::tests::payload_serialize_roundtrip` 必须加载 `agent/packages/schema/samples/realm-vision-*.sample.json` 同一份 fixture。

### 10.3 Client (gradle test)

| 测试类 | 用例数 | 抓手 |
|------|--------|------|
| `RealmVisionPlannerTest::base_per_realm` | 6 | 输出 fog/vignette/tint 数值断言 |
| `RealmVisionPlannerTest::clamp_to_render_distance` | ≥ 4 | fog end clamp 到 server view_distance × 16 - 4 |
| `RealmVisionPlannerTest::with_status_modifier` | ≥ 10 | 入定/入魔等叠加 |
| `RealmVisionPlannerTest::with_env_modifier` | ≥ 15 | 夜/雨/负灵域叠加 |
| `RealmVisionPlannerTest::with_temporal_event` | 4 | 顿悟金光 + 基础叠加（与 VisualEffectController 协调） |
| `RealmVisionPlannerTest::smooth_interpolation` | ≥ 8 | 起→终 + tick → 中间帧 |
| `RealmVisionStateReducerTest::apply_payload` | ≥ 8 | server payload → state |
| `RealmVisionStateReducerTest::apply_breakthrough_diff` | ≥ 4 | 多次 payload 累积 |
| `RealmVisionInterpolatorTest` | ≥ 8 | 起 / 终 / tick → 中间值 |
| `PerceptionEdgeProjectorTest::inside_view` | 4 | 视野内不画 |
| `PerceptionEdgeProjectorTest::edge_left_right_top_bottom` | 4 | 视野外四个边 |
| `PerceptionEdgeProjectorTest::corners` | 4 | 四个角 |
| `PerceptionEdgeProjectorTest::behind_camera` | 2 | 摄像机背后投影 |
| `PerceptionEdgeProjectorTest::priority_overflow` | 2 | >3 个同方向只画 3 个 |
| `MixinBgRendererSinkTest::sink_receives_planner_output` | ≥ 6 | RecordingSink 断言 fog 参数调用序列 |
| `MixinBgRendererSinkTest::sink_call_count_matches_frames` | 1 | 每帧调用 sink 一次 |
| `ClientRenderDistanceAdvisorTest::toast_under_threshold` | 1 | < 12 触发 toast |
| `ClientRenderDistanceAdvisorTest::no_toast_above_threshold` | 1 | ≥ 16 不触发 |
| `RealmVisionFixtureTest` | ≥ 11 | 加载 schema/samples/*.sample.json → reducer → planner → 期望 RealmVisionCommand |

**总计 ≥ 99 用例**。

### 10.4 端到端 (脚本)

最后 1cm GL 胶水（`GlFogParamsSink` ≤ 5 行调 `RenderSystem.setShaderFog*`）不能 headless 测试，但每 Phase 验收时人工跑一次：

```bash
bash scripts/dev-reload.sh
# 然后 ./gradlew runClient + 服务端 cargo run + agent npm start
# Phase 1: /setrealm awaken / induce / condense / solidify / spirit / void 切换观察视距 + fog + vignette + tint 变化
# Phase 2: 凝脉境玩家面对墙，墙后另一玩家 → 边缘出现淡白圆点 + 蓝色波纹
# Phase 3: 化虚境进负灵域 → 视距骤降 + vignette 弹性收缩
```

### 10.5 测试指令汇总（每个 Phase 必须红→绿）

```bash
# Server
cd server && cargo test --lib realm_vision spiritual_sense
cd server && cargo test --lib schema::realm_vision
cd server && cargo test --lib schema::server_data  # 包含新 variant

# Schema
cd agent/packages/schema && npm test -- realm-vision spiritual-sense

# Client
cd client && ./gradlew test --tests "*RealmVision*"
cd client && ./gradlew test --tests "*PerceptionEdge*"
cd client && ./gradlew test --tests "*MixinBgRendererSink*"
cd client && ./gradlew test --tests "*RealmVisionFixture*"

# 全量（push 前必跑）
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd agent && npm run build && cd packages/schema && npm test
cd client && ./gradlew test build

# Phase 验收人工 demo
bash scripts/dev-reload.sh
```

**`/consume-plan` 合并条件**：上述测试指令全部 ✅ + 对应 Phase 端到端 demo 人工验证一次。

---

## §11 进度日志

- 2026-04-30：v1.1 起草。基于 v1 模板态进行整合：去 Iris 依赖、补接入面 checklist、给每 Phase 加可 grep 抓手、新增 §10 测试矩阵。当前仓库内"perception"字样仅指 narration `NarrationStyle::Perception` 旁白样式与 `InsightCategory::Perception`（顿悟类别 + `UnlockedPerceptions` 解锁集合），均与本 plan 无关；本 plan 实装时一律使用 `RealmVision` / `SpiritualSense` 词根避免歧义。Phase 0/1/2/3 全部待启动。
- 2026-04-30：根据用户决策落地三项开放问题决议（视距归属：worldview 写两端 + plan 写中间 / 化虚神识半径：三圈分层 / 隐匿模糊化：境界差三态 hook）+ 视距表现实化重构。基于 Bong server `server/src/preview/mod.rs::boost_view_distance_for_preview` 提示——Valence default `ViewDistance` 仅 2 chunks，preview 模式才提到 32 chunks（vanilla 协议上限）——确认 800m 化虚视距不现实（需要 50 chunks 超出协议）。视距表全替换为 30/50/80/120/180/240m，server `ViewDistance` 按境界绑（4/6/8/12/16/20 chunks），化虚境视距优势削弱由 §5.4 化虚境视觉补偿（粒子 + tint + post-fx 锐化 + 神识全图）补足。新增 §5.4 化虚境视觉补偿、`ClientRenderDistanceAdvisor` toast 引导、`ViewDistanceRampSystem` 平滑过渡。`FLOOR_CLAMP_M` 从 30m 调整到 15m。测试矩阵 server 部分从 ≥ 105 用例扩到 ≥ 124 用例，client 部分从 ≥ 95 扩到 ≥ 99 用例。
