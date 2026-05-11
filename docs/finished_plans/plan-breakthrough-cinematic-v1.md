# Bong · plan-breakthrough-cinematic-v1 · 骨架

突破/修炼电影化奇观——在 `plan-vfx-wiring-v1`（突破 VFX 接线）+ `plan-audio-world-v1`（修炼音效）+ `plan-hud-polish-v1`（全屏事件特效）✅ active 基础上拓展。vfx-wiring-v1 P0 已有 `BreakthroughPillarPlayer` 光柱 + 突破失败碎裂粒子 + 经脉打通光路；audio-world-v1 P1 已有修炼冥想音 + 经脉打通清音；hud-polish-v1 P2 已有突破成功金色边框 flash + 渡劫紫电纹路。但这些是**单次触发效果**——缺乏多阶段叙事编排。玩家静坐 3 分钟突破时只有一条 narration + 一个光柱。本 plan 把每个突破编排成**5 阶段电影化事件**（prelude→charge→catalyze→apex→aftermath），让突破变成全服可感知的灵气奇观。

**世界观锚点**：`worldview.md §三` 六境界突破条件（静坐 + 灵气环境 + 局部循环 → 凝核 → 共鸣 → 渡劫）· `§八` 天道对突破的态度（低境怜悯 / 高境敌视）· `§十七` 突破与季节共振（夏季雷劫可期 / 冬季爆发不足 / 汐转高风险）· `§四` 境界越一级是"可咬一口"——突破瞬间是脆弱的

**library 锚点**：`cultivation-0001 六境要录`（突破体征描述）· `cultivation-0006 经脉浅述`（经脉光路走向）

**前置依赖**：
- `plan-vfx-wiring-v1` 🆕 active → **突破 VFX 基础**（BreakthroughPillarPlayer 光柱 + 失败碎裂粒子 + 吸灵粒子 + 经脉光路。本 plan 编排其触发时序而非重写）
- `plan-audio-world-v1` 🆕 active → **修炼音效基础**（冥想 loop + 经脉清音。本 plan 编排其触发时序 + 追加突破 sequence 专属音效）
- `plan-hud-polish-v1` 🆕 active → **全屏事件特效**（突破金框 flash + 渡劫紫电。本 plan 编排其触发时机）
- `plan-cultivation-v1` ✅ → BreakthroughRequest / Realm 转换事件
- `plan-particle-system-v1` ✅ → BongLineParticle / BongRibbonParticle / BongGroundDecalParticle 渲染基类
- `plan-vfx-v1` ✅ → 屏幕级叠加（HUD 叠色 / FOV / shake）
- `plan-audio-implementation-v1` 🆕 skeleton → 突破 pulse / 共鸣嗡鸣 recipe
- `plan-player-animation-implementation-v1` 🆕 skeleton → 突破姿态动画
- `plan-jiezeq-v1` ✅ → 突破与季节共振 hook
- `plan-season-full-experience-v1` 🆕 skeleton → 季节叠加 cinematic 视觉（P4 定义）

**反向被依赖**：
- `plan-tribulation-v2` 🆕 active → 渡虚劫可叠加突破 cinematic（化虚渡劫是双重 spectacle）

---

## 与各 active plan 的边界

| 维度 | active plan 已做 | 本 plan 拓展 |
|------|-----------------|-------------|
| 突破光柱 | vfx-wiring-v1 P0：`BreakthroughPillarPlayer`（单次 emit） | 编排：5 阶段时序（prelude 微弱→charge 渐强→catalyze 极亮→apex 爆发→aftermath 余韵）|
| 失败粒子 | vfx-wiring-v1 P0：红色碎裂环 | 编排 + 增强：打断 visual 序列（爆散 + 红闪 + 败兴） |
| 经脉光路 | vfx-wiring-v1 P0：`MeridianOpenFlashPlayer`（打通时） | 编排：突破全程经脉循环可见（不是只在打通瞬间）|
| 修炼音效 | audio-world-v1 P1：冥想 loop + 经脉清音 | 编排 sequence：心跳渐快→洪钟→长余韵 |
| 全屏 flash | hud-polish-v1 P2：突破金框 flash 1s | 编排时机：仅在 apex 阶段触发 + 增强参数（按境界不同强度）|
| 吸灵粒子 | vfx-wiring-v1 P0：`CultivationAbsorbPlayer`（打坐时） | 编排：突破 charge 阶段吸灵密度 ×5 + 方向收束（周围→丹田）|
| 全服异象 | 无 | 新增：凝脉+ 突破时全服 5km+ 可见天空光柱 + 灵压波动 |
| 打断张力 | 无 | 新增：cinematic 中被攻击 → 中断序列 + 失败 visual |

---

## 接入面 Checklist

- **进料**：`cultivation::BreakthroughRequest` event / `cultivation::MeridianSystem`（经脉图数据）/ `cultivation::Cultivation { qi_current, qi_max, realm }` / `BreakthroughPillarPlayer`（vfx-wiring-v1 出料，复用） / `CultivationAbsorbPlayer`（vfx-wiring-v1 出料）/ `RealmVisionState` / `SeasonState`
- **出料**：`BreakthroughCinematic` server component（跟踪 5 阶段：prelude/charge/catalyze/apex/aftermath + 当前 tick + 被打断标记）+ `BreakthroughSpectacleRenderer`（client 侧编排器，按阶段调度各 active plan 的 VFX/audio/HUD 触发）+ 每境专用参数 profile（时长/粒子密度/音效 sequence/全服可见距离）
- **跨仓库契约**：server `BreakthroughCinematic` 阶段机（server 权威推进各阶段 tick）→ `BreakthroughCinematicS2c { phase, tick, realm_from, realm_to }` packet → client `BreakthroughSpectacleRenderer`（纯表演层）/ agent 订阅 `bong:breakthrough_cinematic` → 同步 narration

---

## §0 设计轴心

- [ ] **编排者不是实现者**：本 plan 不重写 VFX/audio/HUD——而是定义 5 阶段时序，在每个阶段调度已有系统
- [ ] **每境不同视觉**：不同境界突破的粒子密度/光效强度/音效/时长各不相同（醒灵→引气 30s 轻盈 / 通灵→化虚 3min 天地异变）
- [ ] **全服可见异象**：凝脉+ 突破在突破点生成天空光柱/灵压波动，5km 内可见——让修仙世界的事件有"社会影响"
- [ ] **可打断 = 高张力**：突破中其他玩家/NPC 可攻击打断 → 突破方在 cinematic 最脆弱时被偷袭的叙事张力
- [ ] **agent narration 与 cinematic 双轨同步**：天道旁白 + 视觉奇观同时推进——prelude 时天道轻描淡写，apex 时天道或沉默或嘲讽

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `BreakthroughCinematic` 状态机 + `BreakthroughSpectacleRenderer` 编排骨架 + 醒灵→引气完整 cinematic | ⬜ |
| P1 | 引气→凝脉 + 凝脉→固元 cinematic | ⬜ |
| P2 | 固元→通灵 + 通灵→化虚 cinematic + 全服异象 | ⬜ |
| P3 | 打断/失败/成功 分支 visual | ⬜ |
| P4 | 季节联动 + 动画联动 | ⬜ |
| P5 | 饱和化测试 | ⬜ |

---

## P0 — 状态机 + 骨架 + 醒灵→引气 ⬜

### 交付物

1. **`BreakthroughCinematic` server component**（`server/src/cultivation/breakthrough_cinematic.rs`）
   - 5 阶段枚举：`Prelude` / `Charge` / `Catalyze` / `Apex` / `Aftermath`
   - 每阶段有 duration_ticks（按 realm transition 参数化）
   - server 权威推进：每 tick 递增 phase_tick → 到 duration → 切下一 phase
   - 被攻击打断：`HitEvent` 命中突破中实体 → `BreakthroughCinematic.interrupted = true` → 跳到 Aftermath(FAIL)
   - emit `BreakthroughCinematicS2c { phase, phase_tick, realm_from, realm_to, interrupted }` 每阶段切换时

2. **`BreakthroughSpectacleRenderer` client 编排器**（`client/src/main/java/com/bong/client/cultivation/BreakthroughSpectacleRenderer.java`）
   - 消费 `BreakthroughCinematicS2c` → 按 phase 调度：
     - **Prelude**：吸灵粒子密度 ×2（调 `CultivationAbsorbPlayer` 参数）+ FOV 微缩 -2° + 心跳音效开始
     - **Charge**：吸灵密度 ×5 + 方向收束（粒子轨迹从散射变为汇聚丹田方向）+ 经脉循环光路可见（调 `MeridianOpenFlashPlayer` 持续模式）+ 心跳加速
     - **Catalyze**：光柱开始（调 `BreakthroughPillarPlayer` 渐亮模式）+ 地面灵压波纹（`BongGroundDecalParticle` 扩散环）+ 周围 entity 微推（风压感）
     - **Apex**：光柱全亮 + 全屏 flash（调 hud-polish-v1 金框 flash）+ 洪钟音效 + agent narration 触发
     - **Aftermath(SUCCESS)**：金色粒子雨 + 光柱渐灭 + HUD 新境界闪烁 + 余韵音
     - **Aftermath(FAIL)**：粒子爆散 + 红闪 + 光柱碎裂 + 败兴音
   - 编排器不自己渲染任何粒子——全部通过调已有 VFX player 的参数/触发完成

3. **醒灵→引气 cinematic 完整实现**
   - 参数 profile：
     - Prelude: 60 tick (3s)
     - Charge: 200 tick (10s)
     - Catalyze: 100 tick (5s)
     - Apex: 40 tick (2s)
     - Aftermath: 120 tick (6s)
     - 总计 ~26s（低境突破较短——worldview §三 醒灵→引气最简单）
   - 全服可见距离：256 block（低境不惊天动地）
   - 视觉基调：轻盈、清新——灵气涡旋为主调

4. **`BreakthroughPillarPlayer` 重构**
   - 原有：单次 emit 光柱 → 固定参数
   - 重构：追加 `setIntensity(float)` / `setColor(int)` / `setHeight(float)` API → 供编排器按 phase 动态调参

### 验收抓手

- 测试：`server::cultivation::tests::cinematic_phase_progression` / `server::cultivation::tests::cinematic_interrupted_by_hit` / `client::cultivation::tests::spectacle_renderer_dispatches_by_phase`
- 手动：打坐突破醒灵→引气 → prelude 灵气汇聚 → charge 经脉循环 → catalyze 光柱升起 → apex 金框 flash → aftermath 粒子雨 → 全程 ~26s

---

## P1 — 引气→凝脉 + 凝脉→固元 ⬜

### 交付物

1. **引气→凝脉 cinematic profile**
   - 总时长 ~50s（中等境界，更庄重）
   - 视觉主调：经脉光路沿 12 正经粒子流（`BongLineParticle` 从四肢→丹田，12 条对应 12 正经，tint 青白 #88CCDD）
   - Charge 阶段：经脉循环光带（沿身体表面光路循环流动——不是单次闪，是持续循环）
   - Catalyze 阶段：局部循环完成 → 体表灵气凝聚成薄层（微光 shell——`BongSpriteParticle` 球面分布 × 24 紧贴身体）
   - Apex：凝脉瞬间光壳碎裂（碎片向外飞散 + 内部新的稳定光路闪现）+ 全屏 tint 青白 0.3s
   - 全服距离：512 block

2. **凝脉→固元 cinematic profile**
   - 总时长 ~80s（高境界，仪式感重）
   - 视觉主调：真元凝核（丹田位置出现半透明球体 → 从 alpha 0.1 渐变为 alpha 0.8 不透明）
   - Charge：周围灵气加速汇聚 + 地面出现灵压圈（`BongGroundDecalParticle` 同心圆 ×3，向内收缩）
   - Catalyze：凝核球体从半透明渐变实体 + 灵眼坐标闪光（如果 zone 有灵眼）
   - Apex：核心凝固 → 全身金色爆发（粒子 × 48 球面向外）+ 洪钟音 + 地面震波
   - 全服距离：1024 block

### 验收抓手

- 测试：各 cinematic profile 参数加载 + 阶段时长
- 手动：引气→凝脉 → 经脉光路循环 → 凝脉→固元 → 凝核球体渐实 → 金色爆发

---

## P2 — 固元→通灵 + 通灵→化虚 + 全服异象 ⬜

### 交付物

1. **固元→通灵 cinematic profile**
   - 总时长 ~120s（高境，天地共鸣级别）
   - Prelude：周围 64 格内所有 NPC 注意力转向突破者（server emit NPC mood 切 ALERT）
   - Charge：天空逐渐变色（sky_tint 向金色偏移 → 通过 `ZoneAtmosphereRenderer` API）
   - Catalyze：天地光柱（vfx-wiring-v1 已有 → 高度从 16→64 block 渐升）+ 远方可见（5km）
   - Apex：全服广播 narration（agent emit：某方向有人正在通灵）+ 全屏白闪 0.5s + 洪钟音（3 层叠加）
   - Aftermath：天空恢复 + 光柱渐灭 → 突破者周围 8 格灵气浓度短暂 +50%（灵气被挤出）
   - 全服距离：5000 block（全图可见天空异变）

2. **通灵→化虚 cinematic profile**
   - 总时长 ~180s（3min，最高级 spectacle）
   - Prelude：全服 narration "天道注意到了什么"（不说谁在突破）→ 天空黑云聚集（sky_tint 暗化 + fog_density +0.2）
   - Charge：渡虚劫序列开始（与 plan-tribulation-v2 叠加）→ 天劫雷 + 突破光柱同时存在
   - Catalyze：天地颤抖（全服 camera micro shake 0.02 intensity 持续——"世界感受到了"）
   - Apex：全服异象 10km（天空裂缝粒子 + 远方看到光柱穿云）+ agent narration "此地，有人在窥探天道"
   - Aftermath(SUCCESS)：天空裂缝愈合 + 10s 内全服灵气浓度微降 0.1（worldview §二 守恒——突破消耗大量灵气）
   - Aftermath(FAIL)：天空黑云消散 + agent 冷漠 narration "不过如此"

3. **全服异象渲染**（`client/src/main/java/com/bong/client/cultivation/DistantBreakthroughRenderer.java`）
   - 远距离（> 64 block）玩家看到的简化版视觉：
     - 天空方向出现光柱（billboard sprite，大小按距离缩放）
     - 天空该方向颜色微变
   - 消费 `BreakthroughCinematicS2c` 的 world_pos → 计算方向 → 天空 billboard
   - 通灵+才全服可见（引气/凝脉/固元仅附近可见）

### 验收抓手

- 测试：`server::cultivation::tests::tongling_cinematic_npc_alert` / `client::cultivation::tests::distant_breakthrough_billboard` / `server::cultivation::tests::huaxu_cinematic_global_qi_drain`
- 手动：远距离（1000+ block）观察另一玩家通灵突破 → 天空方向出现光柱 → 全服 narration

---

## P3 — 打断/失败/成功分支 ⬜

### 交付物

1. **打断 visual 序列**
   - cinematic 任意 phase 中被 HitEvent 命中 → `BreakthroughCinematic.interrupted = true`
   - 编排器立即执行：
     - 所有粒子瞬间爆散（VFX player 参数：lifetime → 0 + velocity ×5 向外）
     - 光柱碎裂（从中间裂开 → 碎片粒子 × 16 向外飞散）
     - 红色 screen flash 0.3s（`OverlayQuadRenderer` 红色 alpha 0.4）
     - camera shake intensity 0.5, 10 tick
     - 音效：`breakthrough_interrupted.json`（`minecraft:block.glass.break`(pitch 0.5) + `minecraft:entity.player.hurt`(pitch 0.6)）
   - 之后进入 Aftermath(FAIL)

2. **正常失败 visual**（非打断，自然失败——灵气不足/条件不满足）
   - 比打断更温和：粒子逐渐消散（不爆散）+ 光柱渐灭 + 灰色 screen tint 1s
   - agent narration："灵气未能凝聚。天地并不在意。"（低境）/ "你还不够格。"（高境天道敌视）

3. **成功庆祝 visual**
   - apex → aftermath(SUCCESS)：
     - 金色粒子雨（`BongSpriteParticle` `enlightenment_dust` tint #FFD700 × 32，从 y+10 缓慢下落，lifetime 60 tick）
     - HUD 新境界名称 flash（屏幕中央大字 "引气期" → 1s fade out，金色，配合 hud-polish-v1 toast 系统）
     - 光柱余韵 10s 渐灭
     - 音效余韵：`minecraft:block.amethyst_block.chime`(pitch 0.5, volume 0.2) loop 3 次 → fade

4. **围观者视角 HUD**
   - 突破者 64 格内其他玩家：HUD 事件流显示"有人正在突破"
   - apex 时围观者：screen tint 微金（alpha 0.05）——"灵气波动影响到你了"
   - 与 npc-interaction-polish-v1 配合：NPC mood 切 ALERT + 气泡 "…有人在突破。"

### 验收抓手

- 测试：`server::cultivation::tests::interrupt_jumps_to_fail_aftermath` / `client::cultivation::tests::interrupt_particle_burst` / `client::cultivation::tests::success_golden_rain`
- 手动：突破中被攻击 → 粒子爆散+红闪+碎裂 → 重新突破成功 → 金色粒子雨+大字+余韵

---

## P4 — 季节联动 + 动画联动 ⬜

### 交付物

1. **季节叠加参数**
   - 编排器在每阶段检查 `SeasonState.phase` → 叠加参数（由 season-full-experience-v1 P4 定义，本 plan 实现消费端）：
     - 炎汐：粒子 tint +金色 hue / apex 追加雷暴粒子 / 成功后 fog 瞬清
     - 凝汐：粒子 tint +冷白 / apex 追加冰晶折射 / 成功后周围雪粒停滞
     - 汐转：粒子 tint 闪烁不定 / 屏幕 noise ×2 / HUD 警告 pulse

2. **动画联动**
   - 编排器在每阶段 emit `AnimationTrigger`（调 player-animation-implementation-v1 出料）：
     - Prelude：`meditate_sit`（已有）
     - Charge：`meditate_sit` + body 微颤（追加 EXPRESSION 层动画）
     - Catalyze/Apex：按境界选择 `breakthrough_yinqi/ningmai/guyuan/tongling`
     - Aftermath(SUCCESS)：缓慢站起 + 双臂微展（从 breakthrough 动画 → idle crossfade）
     - Aftermath(FAIL)：受击后仰 `hurt_stagger` → 倒地 `death_collapse` 微版（不倒地，但大幅前倾）

3. **与 agent narration 时序同步**
   - server 在每 phase 切换时向 agent 发 `bong:breakthrough_cinematic` Redis event
   - agent 按 phase 选择 narration 模板：
     - Prelude："此处灵气开始聚拢。"
     - Charge（低境）："一只蝼蚁在试图汲取天地灵气。可以。"
     - Charge（高境）："天道注意到了。"
     - Apex（成功）："…通过了。"（低境温和）/ 沉默（高境——天道不屑评价）
     - Apex（失败）："不过如此。"

### 验收抓手

- 测试：`client::cultivation::tests::season_modifies_cinematic_params` / `server::cultivation::tests::cinematic_emits_agent_event`
- 手动：夏季突破 → 金色调 + 雷暴 → 冬季 → 冷白 + 冰晶 → 突破全程有对应姿态动画 → agent narration 按阶段出现

---

## P5 — 饱和化测试 ⬜

### 交付物

1. **全矩阵覆盖**
   - 5 境突破 × 2 结果（成功/失败）× 3 季节 × 2 打断方式（自然/被攻击）= 60 组合
   - 每组合完整走完 5 阶段 → 确认视觉/音效/narration/动画 sequence 正确

2. **多客户端围观**
   - 3 玩家围观 1 人突破：所有围观者看到同步的光柱/粒子/天空变化
   - 远距离围观者（1000+ block）：看到天空 billboard + 收到 narration

3. **性能**
   - cinematic 全程粒子开销 < 3ms（最重的 apex 阶段）
   - 光柱 + 粒子雨 + 全屏 flash 叠加不掉帧

### 验收抓手

- 自动化：`scripts/breakthrough_cinematic_test.sh`（遍历 60 组合 + 截图）
- 多人同步：3 client + 1 突破者 → 帧率日志

---

## Finish Evidence

- **落地清单**：
  - P0/P1/P2/P3/P4：`server/src/cultivation/breakthrough_cinematic.rs` 落地 `BreakthroughCinematic` component、5 阶段状态机、5 境 transition profile、打断跳转、`BreakthroughCinematicS2cV1` 下发、`bong:breakthrough_cinematic` agent event、以及 `bong:vfx_event` 复用触发。
  - P0/P1/P2/P3/P4：`client/src/main/java/com/bong/client/cultivation/BreakthroughCinematicPayload.java` / `BreakthroughSpectacleRenderer.java` / `DistantBreakthroughRenderer.java` 与 `client/src/main/java/com/bong/client/network/BreakthroughCinematicHandler.java` 落地 client 编排、远距 billboard 计划、成功/失败/打断分支、季节强度叠加与动画/audio recipe id 输出。
  - P4：`agent/packages/schema/src/breakthrough-cinematic.ts`、`agent/packages/tiandao/src/breakthrough-cinematic-narration.ts`、`agent/packages/tiandao/src/main.ts` 落地 TypeBox schema、Redis channel `bong:breakthrough_cinematic`、天道旁白 runtime 与 fallback narration。
  - P5：`scripts/breakthrough_cinematic_test.sh` 串起 server/client/agent cinematic contract 验证；`agent/packages/schema/generated/breakthrough-cinematic-event-v1.json` 与 `server-data-v1.json` 已刷新。
- **关键 commit**：
  - `7565ec60f` · 2026-05-12 · `实现突破 cinematic 阶段协议`
  - `5edb14877` · 2026-05-12 · `接入突破 cinematic 客户端编排`
  - `bd8ee878c` · 2026-05-12 · `同步突破 cinematic 天道旁白`
  - `f795b6f88` · 2026-05-12 · `补充突破 cinematic 验证脚本`
- **测试结果**：
  - `cd server && cargo fmt --check`
  - `cd server && cargo clippy --all-targets -- -D warnings`
  - `cd server && cargo test` -> 4431 passed
  - `cd client && JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn ./gradlew --no-daemon clean test build`
  - `cd agent && npm run build`
  - `cd agent && npm test -w @bong/schema` -> 377 passed
  - `cd agent && npm test -w @bong/tiandao` -> 357 passed
  - `scripts/breakthrough_cinematic_test.sh` -> server 4 passed, client targeted tests passed, schema 377 passed, tiandao cinematic 3 passed
- **跨仓库核验**：
  - server：`BreakthroughCinematic` / `BreakthroughCinematicS2cV1` / `CH_BREAKTHROUGH_CINEMATIC` / `RedisOutbound::BreakthroughCinematic`
  - client：`BreakthroughCinematicPayload` / `BreakthroughSpectacleRenderer` / `DistantBreakthroughRenderer` / `BreakthroughCinematicHandler`
  - agent：`BreakthroughCinematicEventV1` / `BreakthroughCinematicNarrationRuntime` / `CHANNELS.BREAKTHROUGH_CINEMATIC`
- **遗留 / 后续**：跨玩家同时突破的光柱合并/干扰策略、化虚渡虚劫与 `plan-tribulation-v2` 的优先级仲裁、真实多人截图/帧率日志仍留给后续联调场景；本 plan 已锁定 server-authoritative cinematic contract 与三栈消费路径。
