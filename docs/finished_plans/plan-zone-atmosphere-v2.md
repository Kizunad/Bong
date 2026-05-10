# Bong · plan-zone-atmosphere-v2

区域氛围视觉识别系统——在 `plan-vfx-wiring-v1`（环境粒子接线）+ `plan-audio-world-v1`（区域 ambient 音效）✅ active 基础上拓展。vfx-wiring-v1 已接线战斗/修炼/产出/社交/死亡/状态效果的事件驱动 VFX，audio-world-v1 已建 6 区域 ambient loop + 昼夜 crossfade——但两者都是**事件触发型或 loop 型**，不解决 zone 之间视觉差异的问题。当前走进血谷和走出灵泉湿地在视觉上几乎一样（除了灵气条数字变化）。本 plan 给每个 zone 赋予**持续的视觉身份**：灵压雾色分层、环境粒子密度、zone 边界过渡特效、负压区视觉扭曲。让玩家不看 HUD 就知道自己进了哪个区。

**世界观锚点**：`worldview.md §二` 灵压三态（馈赠区 / 死域 / 负灵域）视觉差异 · `§十三` 6 区域各不相同的威胁/资源/灵气 · `§十七·5 类地形季节响应`（死域恒 0、渊口寒气带冬 ×1.3 等）· `§七` 残灰方块（踩上减速+留脚印）→ 视觉上可区分死域边缘

**library 锚点**：`world-0002 末法纪略`（各区域首次描述）· `ecology-0005 异兽三形考`（不同 zone 的生态暗示）

**前置依赖**：
- `plan-vfx-wiring-v1` 🆕 active → 环境状态 VFX（中毒雾 / 死亡灰化 / 经脉闪烁等事件 VFX 已有，本 plan 叠加 zone 持续粒子层）
- `plan-audio-world-v1` 🆕 active → 6 区域 ambient loop（本 plan 的 zone profile 与 audio-world 的 ambient 一一对应，视听协同）
- `plan-vfx-v1` ✅ → 屏幕叠加层
- `plan-particle-system-v1` ✅ → 环境粒子基类
- `plan-HUD-v1` ✅ → ZoneHudRenderer（zone 信息显示）
- `plan-realm-vision` (impl) ✅ → RealmVisionFogController / FogParamsSink（本 plan 在此之上叠加 zone-specific fog color）
- `plan-jiezeq-v1` ✅ → 季节 fog/sky color 覆盖（P4 季节联动）

**反向被依赖**：
- `plan-combat-gamefeel-v1` 🆕 → zone 内战斗 PVP 视野受 zone fog 影响
- `plan-breakthrough-cinematic-v1` 🆕 → 突破光柱在 zone atmosphere 中更突出
- `plan-hud-immersion-v2` 🆕 → 灵压雷达配色依据 zone profile
- `plan-season-full-experience-v1` 🆕 → 季节天空/fog 叠加到 zone profile

---

## 与 vfx-wiring-v1 + audio-world-v1 的边界

| 维度 | vfx-wiring-v1 已做 | audio-world-v1 已做 | 本 plan 拓展 |
|------|-------------------|-------------------|-------------|
| 环境粒子 | 中毒绿雾/虚弱灰雾等**状态触发型**粒子 | — | zone 持续**环境型**粒子（山间薄雾/铁锈尘埃/萤火/风沙） |
| fog | — | — | zone-specific fog color/density（叠加在 RealmVisionFogController 之上） |
| sky | — | — | zone-specific sky tint（晴天/阴天/灰白/暗红） |
| ambient | — | 6 区域 ambient loop + 昼夜 | 不碰 ambient 音。仅在 zone profile 中引用对应 ambient recipe ID |
| 死域/负灵域 | 虚弱灰雾（状态效果）| 无特殊 | 死域全饱和度 -50% + 远景 cutoff / 负灵域紫黑扭曲 |
| zone 边界 | 无 | 无 | 150 格过渡带 fog/particle/sky lerp |
| 坍缩渊 | 无 | TSY ambient | TSY 内部分层 fog（浅/中/深）+ 塌缩视觉序列 |

---

## 接入面 Checklist

- **进料**：`ZoneEnvironment` component（plan-zone-environment-v1 ✅）/ `qi_physics::zone::ZoneQiPressure` / `RealmVisionFogController`（plan-realm-vision ✅）/ `SeasonState`（plan-jiezeq-v1 ✅）/ zone 坐标/名称
- **出料**：`ZoneAtmosphereProfile`（每 zone 6 参数: fog_color / fog_density / ambient_particle_type / sky_tint / entry_transition_fx / associated_ambient_recipe_id）+ `ZoneAtmosphereRenderer`（按 profile 动态混合 fog+particles+sky）+ zone 边界过渡带系统（150 格渐变）+ 6+ zone profile
- **跨仓库契约**：server 不动——纯 client 侧按 `ZoneEnvironment.zone_id` 选 profile

---

## §0 设计轴心

- [ ] **每个 zone 看起来不同**：玩家不需要看 HUD 就知道自己进了哪个区
- [ ] **灵压 ⇄ 视觉耦合**：高灵气区 = 清透/微金 fog；死域 = 灰白/远景 fade；负灵域 = 紫黑/扭曲
- [ ] **zone 边界 = 过渡带而非硬切**：150 格渐变 lerp，避免"一步天堂一步地狱"的硬边界
- [ ] **残灰方块足迹**：死域/馈赠区边缘地面方块退化为残灰方块时，视觉可见
- [ ] **与 audio-world-v1 视听协同**：每个 zone profile 声明对应的 `ambient_recipe_id`，视觉切换与 ambient 切换同步

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | ZoneAtmosphereProfile 数据结构 + Renderer 骨架 + 初醒原 / 青云残峰 profile + 过渡带 | ✅ 2026-05-10 |
| P1 | 血谷 / 灵泉湿地 / 北荒 / 幽暗地穴 profile | ✅ 2026-05-10 |
| P2 | 死域 / 负灵域特殊视觉 + 残灰方块足迹 | ✅ 2026-05-10 |
| P3 | 坍缩渊分层 atmosphere + 塌缩视觉序列 | ✅ 2026-05-10 |
| P4 | 季节联动（SeasonState → zone profile 动态覆盖） | ✅ 2026-05-10 |
| P5 | 性能压测（6 zone × 3 季节 × fog+particle 开销） | ✅ 2026-05-10 |

---

## P0 — 数据结构 + 骨架 + 首批 profile ✅ 2026-05-10

### 交付物

1. **`ZoneAtmosphereProfile`**（`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereProfile.java`）
   - 6 参数：
     ```
     fog_color: int (ARGB)
     fog_density: float (0.0-1.0, 越大越浓)
     ambient_particle: ParticleConfig (type + tint + density + speed)
     sky_tint: int (ARGB, 叠加到 vanilla sky)
     entry_transition_fx: TransitionFx (enum: NONE / FADE / MIST_BURST / WIND_GUST)
     ambient_recipe_id: String (对应 audio-world-v1 的 recipe)
     ```
   - JSON 配置文件：`client/src/main/resources/assets/bong/atmosphere/` 每 zone 一个 JSON
   - hot-reload（开发期改 JSON 不重启 client）

2. **`ZoneAtmosphereRenderer`**（`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereRenderer.java`）
   - `WorldRenderEvents.AFTER_SETUP` 钩子
   - 读取当前玩家 `ZoneEnvironment.zone_id` → 查 profile → 设置 fog + spawn particle + tint sky
   - 叠加到 `RealmVisionFogController` 之上（realm-vision 管境界 fog，本 renderer 管 zone fog，两者混合：zone fog color lerp with realm fog color，density 取 max）

3. **初醒原 profile**（`atmosphere/spawn_plain.json`）
   - fog_color: #B0C4DE（淡灰蓝——荒凉但不压抑）
   - fog_density: 0.15（薄）
   - ambient_particle: `cloud256_dust` tint #D0D0D0, density 0.5, drift speed 0.01（微弱浮尘）
   - sky_tint: #E8E8F0（微冷色天空）
   - entry_transition_fx: NONE（起始区不需要过渡）
   - ambient_recipe_id: `ambient_spawn_plain`

4. **青云残峰 profile**（`atmosphere/qingyun_peaks.json`）
   - fog_color: #8090A0（青灰——山间雾气）
   - fog_density: 0.3（中等偏浓——群山间雾多）
   - ambient_particle: `cloud256_dust` tint #C0C8D0, density 1.5, drift speed 0.02 + vertical drift -0.005（薄雾向上缓升）
   - sky_tint: #C0C8D8（微阴天）
   - entry_transition_fx: MIST_BURST（进入时薄雾从脚下升起 0.5s）
   - ambient_recipe_id: `ambient_qingyun_peaks`

5. **Zone 过渡带系统**（`ZoneBoundaryTransition.java`）
   - 两 zone 之间 150 格渐变区域
   - fog_color: lerp(zone_A.fog_color, zone_B.fog_color, t)（t = 在过渡带中的位置 0.0-1.0）
   - fog_density: lerp
   - ambient_particle: 混合两 zone 粒子，density 各按 t 缩放
   - sky_tint: lerp
   - 过渡位置来自 worldgen zone boundary 数据（`ZoneEnvironment.boundary_distance`）

### 验收抓手

- 测试：`client::atmosphere::tests::profile_loads_from_json` / `client::atmosphere::tests::fog_overlays_realm_vision` / `client::atmosphere::tests::boundary_lerp_150_blocks` / `client::atmosphere::tests::hot_reload_updates_fog`
- 手动：在初醒原 → 薄雾浮尘 + 灰蓝天 → 走向青云残峰 → 150 格内逐渐雾浓 + 天色阴 + 脚下升雾 → 到达后山间厚雾

---

## P1 — 其余 4 zone profile ✅ 2026-05-10

### 交付物

1. **血谷 profile**（`atmosphere/blood_valley.json`）
   - fog_color: #5A2020（暗红——铁锈/血色）
   - fog_density: 0.35（浓——压迫感）
   - ambient_particle: `cloud256_dust` tint #8B4040 density 2.0 + `tribulation_spark` tint #FF4444 density 0.3 interval 60tick（偶发电光粒子——暗示灵压不稳）
   - sky_tint: #604040（暗天——压抑）
   - entry_transition_fx: WIND_GUST（进入时热风扑面 0.3s + 灰尘飞扬）

2. **灵泉湿地 profile**（`atmosphere/spring_marsh.json`）
   - fog_color: #A0C8A0（淡绿——潮湿清新）
   - fog_density: 0.25
   - ambient_particle: `lingqi_ripple` tint #88CC88 density 1.0（水雾）+ `enlightenment_dust` tint #CCFFCC density 0.5（萤火微光）
   - sky_tint: #D0E8D0（湿润青天）
   - entry_transition_fx: MIST_BURST（水雾从地面升起）

3. **北荒 profile**（`atmosphere/north_wastes.json`）
   - fog_color: #909090（灰白——荒芜）
   - fog_density: 0.5（浓——风沙遮天）
   - ambient_particle: `cloud256_dust` tint #A0A0A0 density 3.0 drift_speed 0.05（风沙——密度高速度快）
   - sky_tint: #B0B0B0（灰白死天——没有颜色的天空）
   - entry_transition_fx: WIND_GUST（寒风扑面 + 沙粒）

4. **荒野/过渡区 profile**（`atmosphere/wilderness.json`）
   - fog_color: #C0C0B0（暖灰——中性）
   - fog_density: 0.1（稀薄——开阔荒原）
   - ambient_particle: `cloud256_dust` tint #B0B0A0 density 0.3（微弱浮尘）
   - sky_tint: #D8D8D0（中性天色）
   - entry_transition_fx: FADE（淡入淡出 0.5s）

### 验收抓手

- 测试：各 profile JSON schema 校验 + `client::atmosphere::tests::all_zones_have_profile`
- 手动：跑遍 6 区域——每个区域明显看起来不同（不看 HUD 也能辨认）

---

## P2 — 死域/负灵域特殊视觉 + 残灰足迹 ✅ 2026-05-10

### 交付物

1. **死域视觉**（`spirit_qi == 0` 的区域）
   - 全屏饱和度 -50%（DrawContext 后处理 desaturation shader——无需 Iris，纯 quad overlay with desaturated color multiply）
   - 远景 cut-off：150 格外纯白 void（fog_density 1.0 + fog_color #FFFFFF 在 150 格处 hard clip）
   - 环境粒子：极低密度灰色灰烬（`cloud256_dust` tint #808080 density 0.2, drift speed 0.002——几乎静止的落灰）
   - 天空：纯白（sky_tint #F0F0F0——这里的天空没有颜色）

2. **负灵域视觉**（`spirit_qi < 0` 的区域）
   - 屏幕边缘紫黑 vignette（OverlayQuadRenderer gradient #330033 → transparent，intensity = |spirit_qi| × 0.3）
   - 屏幕边缘扭曲 shader（DrawContext quad + noise offset displacement——无需 Iris）
   - 真元被抽吸粒子：身体周围紫色粒子向负灵方向飞去（`BongSpriteParticle` `qi_aura` tint #9944CC × 4 per 20 tick）
   - 环境粒子：深紫色微粒低速飘荡（density 按 |spirit_qi| 缩放）

3. **残灰方块足迹**
   - 死域/馈赠区边缘：地面退化为残灰方块（worldgen 已有残灰层）
   - 玩家踩上残灰方块 → 脚印粒子：灰色灰烬从脚底向上扬（`BongSpriteParticle` `cloud256_dust` tint 灰 × 2）
   - 脚印 decal：地面出现暗色脚印 texture（`BongGroundDecalParticle` tint #555555，30s 消散）
   - NPC 同样留脚印（靠 entity 移动 event 触发，不区分玩家/NPC）

### 验收抓手

- 测试：`client::atmosphere::tests::dead_zone_desaturation_50pct` / `client::atmosphere::tests::negative_qi_vignette_intensity` / `client::atmosphere::tests::ash_footprint_on_step`
- 手动：走入死域 → 世界变灰+远处白雾 → 走入负灵域 → 紫边+扭曲+身周紫色吸流 → 踩残灰 → 灰烬扬起+脚印留下

---

## P3 — 坍缩渊分层 atmosphere ✅ 2026-05-10

### 交付物

1. **TSY 分层 fog**
   - 浅层（tier 1-3）：fog_density 0.3 + fog_color #404050（薄暗雾——能看 50 格）
   - 中层（tier 4-6）：fog_density 0.6 + fog_color #252530（浓暗雾——能看 20 格）
   - 深层（tier 7+）：fog_density 0.9 + fog_color #101015（几乎全黑——能看 8 格）
   - tier 切换时 1s fog lerp（不突变）

2. **TSY 环境粒子**
   - 浅层：偶发灰尘从干尸/遗骸位置飘起（entity-anchored particle）
   - 中层：追加微弱紫色电弧粒子（`BongLineParticle` random 方向，lifetime 5 tick，每 200 tick）
   - 深层：追加负压变形视觉（屏幕轻微呼吸缩放 0.5% 振幅，周期 3s——不晕但有"被呼吸吞吐"的感觉）

3. **塌缩视觉崩溃序列**（坍缩渊倒计时结束）
   - 倒计时 60s：fog 开始缓慢变黑（fog_color lerp → #000000）
   - 倒计时 30s：vignette 收紧 + 地面裂痕 decal 出现
   - 倒计时 10s：全屏大幅晃动（camera shake intensity 0.5 持续）+ 雾完全黑
   - 倒计时 0：全黑 0.5s → 被挤出 TSY → 正常世界 fog 恢复

4. **与 tsy-experience-v1 的协同**
   - tsy-experience-v1（active）负责 portal 模型/VFX、坍缩级联视觉、race-out 警报、遗物发光、搜刮动画
   - 本 plan 负责 TSY 内部**持续 fog/sky/particle**——两者叠加
   - 共享 `TsyPresence.tier` 数据源

### 验收抓手

- 测试：`client::atmosphere::tests::tsy_fog_by_tier` / `client::atmosphere::tests::tsy_deep_breathing_scale` / `client::atmosphere::tests::collapse_visual_sequence_timing`
- 手动：进入坍缩渊 → 浅层暗雾 → 下深层 → 雾越来越浓 → 视野缩到 8 格 → 坍缩开始 → 雾变黑+晃动+裂痕 → 全黑 → 被弹出

---

## P4 — 季节联动 ✅ 2026-05-10

### 交付物

1. **季节覆盖 zone profile**
   - 按 `SeasonState.phase` 动态调 zone profile 参数：
     - 炎汐（夏）：fog_density ×0.8 + sky_tint 微金偏移（+金色 hue 10°）+ ambient_particle speed ×1.3（暖流加速）
     - 凝汐（冬）：fog_density ×1.3 + sky_tint 灰白偏移 + ambient_particle 追加雪粒（`BongSpriteParticle` `cloud256_dust` tint #FFFFFF density 1.0 drift_speed (0, -0.03, 0)——缓慢下落）
     - 汐转（过渡）：fog_density 波动（sin 曲线 ±0.1）+ sky_tint 间歇紫灰闪烁（随机 3-10s 闪一次）

2. **死域不受季节影响**（worldview §十七 第一条）
   - 死域 profile 的季节覆盖全部 skip（硬编码 guard `if zone.is_dead_zone: return`）
   - 余烬死地同理

3. **天气粒子**（季节触发的极端天气）
   - 夏季血谷：雷暴粒子（高空闪光 + 远雷 → 与 audio-world-v1 的 blood_valley ambient 联动）
   - 冬季北荒：暴风雪粒子（fog_density max + 大量雪粒 + 水平 drift speed 0.08——暴风方向）
   - 汐转全域：偶发灵气紊流可见光（`BongLineParticle` 随机方向大尺度线条，lifetime 10 tick，每 60 tick——空中灵气在乱窜）

### 验收抓手

- 测试：`client::atmosphere::tests::summer_reduces_fog_density` / `client::atmosphere::tests::dead_zone_ignores_season` / `client::atmosphere::tests::winter_adds_snow_particle`
- 手动：夏季 → 天色微金 + 雾淡 → 冬季 → 灰白天+浓雾+飘雪 → 汐转 → 天色闪紫+灵气乱窜 → 走入死域 → 不管什么季节都灰白

---

## P5 — 性能压测 ✅ 2026-05-10

### 交付物

1. **ZoneAtmosphere 渲染开销**
   - 6 zone profile 切换 fog/sky lerp 开销 < 0.5ms per frame
   - 过渡带 150 格 lerp 计算 < 0.3ms
   - 最大粒子场景（北荒风沙 density 3.0 + 冬季雪粒 density 1.0）< 2ms

2. **全矩阵覆盖**
   - 6 zone × 3 季节 × 2 极端环境（死域/负灵域）= 36 组合
   - 每组合截图 + 肉眼验证视觉身份可辨

3. **分辨率适配**
   - fog/shader 效果在 1366×768 / 1920×1080 / 2560×1440 表现一致

### 验收抓手

- 自动化：`scripts/atmosphere_matrix_test.sh`（teleport 遍历 36 组合 + 截图）
- 帧率日志：每 zone 30s 停留 + 过渡带 60s 穿越

---

## Finish Evidence

- **落地清单**：
  - P0/P1：`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereProfile.java` / `ZoneAtmosphereProfileParser.java` / `ZoneAtmosphereProfileRegistry.java` / `ZoneBoundaryTransition.java`，以及 `client/src/main/resources/assets/bong/atmosphere/*.json`（`spawn_plain` / `qingyun_peaks` / `blood_valley` / `spring_marsh` / `north_wastes` / `wilderness` / `dark_cavern` / `tsy`）。
  - P2：`ZoneAtmospherePlanner` 的 dead-zone / negative-zone 分支、`AshFootprintTracker`、`ZoneAtmosphereHudPlanner` 的 desaturation / vignette overlay。
  - P3：`ZoneAtmospherePlanner` 的 TSY tier fog、deep breathing、collapse countdown blacken/vignette/camera-shake sequence。
  - P4：`SeasonState` 驱动的 summer / winter / tide-turn profile override，dead-zone skip guard。
  - P5：`ZoneAtmosphereCommand.estimatedFrameCostMs()`、`ZoneAtmosphereTest.atmosphere_matrix_perf_stays_under_budget`、`scripts/atmosphere_matrix_test.sh`。
- **关键 commit**：
  - `d34bdf35e`（2026-05-10）`feat(client): 建立区域氛围 profile 基础`
  - `38b30d9b7`（2026-05-10）`feat(client): 接入区域氛围渲染链路`
  - `48d2268c5`（2026-05-10）`test(client): 增加区域氛围矩阵验证入口`
- **测试结果**：
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$JAVA_HOME/bin:$PATH ./gradlew test --tests com.bong.client.atmosphere.ZoneAtmosphereTest` → PASS
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$JAVA_HOME/bin:$PATH ./gradlew test build` → PASS
  - `bash scripts/atmosphere_matrix_test.sh` → PASS
- **跨仓库核验**：
  - client：`ZoneAtmosphereRenderer.bootstrap()` 在 `BongClient` 启动；`EnvironmentEffectController` tick 更新 atmosphere；`EnvironmentFogController` 将 zone atmosphere 与现有 environment fog 合成；`BongHudOrchestrator` 追加 atmosphere overlay。
  - server/agent：本 plan 不新增协议；继续消费已有 `zone_info` / `SeasonState` / `ZoneEnvironmentStateV1` / TSY extract countdown。
- **遗留 / 后续**：
  - 实机截图和逐分辨率肉眼验收仍依赖 `runClient` 环境；本 PR 提供可重复的 36 组合 JVM 矩阵和脚本入口。
  - 后续新增 zone 只需追加 profile JSON；天气系统独立 plan 可复用 `ZoneAtmosphereProfile.ParticleConfig`。
