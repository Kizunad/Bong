# Bong · plan-death-rebirth-cinematic-v1 · 完成

死亡与重生电影化体验——在 `plan-vfx-wiring-v1`（死亡 VFX）+ `plan-hud-polish-v1`（死亡全屏特效）+ `plan-audio-world-v1`（低 HP 心跳）✅ active 基础上拓展。vfx-wiring-v1 P2 已有 `DeathSoulDissipatePlayer` 魂散粒子 + 灰化 overlay + 遗念文字粒子；hud-polish-v1 P2 已有死亡灰化全屏 1s + 遗念文字渐出；audio-world-v1 P1 已有低 HP 心跳 + 真元耗尽警告。但死亡瞬间到重生在灵龛的完整流程是**零散的碎片效果——不是连贯的叙事**。本 plan 把这些碎片编排成 **6 阶段电影化死亡流程**（predeath→death_moment→roll→insight_overlay→darkness→rebirth），让死亡从"状态跳变"变成"可感知的末法残土学费"。

**世界观锚点**：`worldview.md §十二` 死亡是学费 | 运数/劫数 roll 概率 visible | 遗念真实信息 | 终焉之言 | 重生惩罚（降境 + 真元 0 + 3min 虚弱）· `§四` 37 条血条复合崩溃 → 濒死时的各层崩解应可见 · `§十六` 坍缩渊内死亡额外规则（秘境所得 100% 掉落 + 干尸化）

**library 锚点**：`world-0001 天道口述残编`（死亡 narration 语调）· `peoples-0007 散修百态`（重生者的心理）

**前置依赖**：
- `plan-vfx-wiring-v1` 🆕 active → **死亡 VFX 基础**（DeathSoulDissipatePlayer / 灰化 overlay / 遗念 text particle / 状态效果 VFX）
- `plan-hud-polish-v1` 🆕 active → **死亡全屏特效**（灰化 1s + 遗念渐出 + 全屏事件特效框架）
- `plan-audio-world-v1` 🆕 active → **低 HP/真元 音效**（心跳 + 耗尽警告）
- `plan-death-lifecycle-v1` ✅ → 死亡判定/结算/重生全链路 / DeathInsight 遗念生成
- `plan-lifespan-v1` ✅ → 寿元扣减
- `plan-multi-life-v1` ✅ → 多周目 / 终焉之言
- `plan-vfx-v1` ✅ → 屏幕级叠加
- `plan-audio-implementation-v1` 🆕 skeleton → 死亡/重生音效 recipe
- `plan-particle-system-v1` ✅ → 粒子基类
- `plan-player-animation-implementation-v1` 🆕 skeleton → 死亡倒地 / 重生苏醒动画
- `plan-HUD-v1` ✅ → MiniBodyHudPlanner 伤口剪影

**反向被依赖**：
- `plan-tsy-raceout-v2` 🆕 skeleton → 坍缩渊内死亡特殊 cinematic（塌缩吞噬）

---

## 与各 active plan 的边界

| 维度 | active plan 已做 | 本 plan 拓展 |
|------|-----------------|-------------|
| 魂散粒子 | vfx-wiring-v1 P2：`DeathSoulDissipatePlayer` | 编排触发时机 + 增强（灵魂碎片按伤口位置喷出） |
| 灰化 overlay | vfx-wiring-v1 P2 / hud-polish-v1 P2 | 编排时序（death_moment 阶段触发，不是 instant）|
| 遗念 text | vfx-wiring-v1 P2：遗念文字粒子 | 升级为毛笔字逐笔书写 overlay（不是粒子文字） |
| 低 HP 心跳 | audio-world-v1 P1 | 编排：predeath 阶段心跳渐快 → death_moment 停止 |
| 濒死 vignette | `NearDeathOverlayPlanner`（已有） | 增强：三层崩解可视化（真元闪烁/经脉红光/伤口溢血） |
| 运数 roll | 无 | 新增 Roll UI（概率可见 + 掷签动画） |
| 终焉之言 | 无 | 新增终焉 cinematic（全屏书法字 + 生平归档） |
| 重生流 | server 侧逻辑已有 | 新增重生 cinematic（灵龛 fade-in + 虚弱灰雾） |
| 坍缩渊死亡 | 无 | 新增干尸化 + 秘境 100% 掉落 visual |

---

## 接入面 Checklist

- **进料**：`death_lifecycle::DeathEvent` / `death_lifecycle::RebirthEvent` / `death_lifecycle::NearDeathEvent` / `death_lifecycle::DeathRoll { luck_value, threshold, success }` / `death_lifecycle::DeathInsight` / `DeathSoulDissipatePlayer`（vfx-wiring-v1 出料） / `NearDeathOverlayPlanner`（已有）/ `cultivation::Cultivation` / `cultivation::Wounds` / `lifespan::Lifespan` / `tsy::TsyPresence`
- **出料**：`DeathCinematic` server component（6 阶段状态机）+ `DeathCinematicRenderer`（client 编排器）+ `DeathRollUI`（运数/劫数 roll 可视化）+ `InsightOverlayRenderer`（毛笔字遗念）+ `FinalWordsRenderer`（终焉之言全屏）+ `RebirthCinematicRenderer`（灵龛 fade-in）
- **跨仓库契约**：server `DeathCinematic` 阶段机 → `DeathCinematicS2c { phase, phase_tick, roll_result, insight_text, is_final }` → client 编排器 / agent 订阅 `bong:death_cinematic` → 同步 narration

---

## §0 设计轴心

- [ ] **编排者不是实现者**：魂散粒子/灰化/心跳都复用 active plan 出料，本 plan 定义 6 阶段时序
- [ ] **濒死可视**：HP < 10% 时三层崩解粒子（真元/经脉/体表依次崩溃）——让濒死不只是 vignette 红圈
- [ ] **死亡瞬间 = 画框碎裂**：屏幕玻璃碎裂效果 → 碎片飞散 → 进入 roll
- [ ] **运数 roll = 天意的冷漠呈现**：概率数字 + 掷签/铜钱动画——不是 slot machine，是"天道决定你的命运"
- [ ] **遗念 = 真实信息 + 冷漠语调**：毛笔字逐笔书写（不是弹窗文本）——天道不在乎你死了
- [ ] **重生 ≠ 回到原点**：灵龛 fade-in + 虚弱灰雾 3min + 降境提示——"你付出了学费"

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `DeathCinematic` 状态机 + `DeathCinematicRenderer` 编排骨架 + 通用死亡→重生全流程 | ✅ 2026-05-12 |
| P1 | 濒死三层崩解 + 画框碎裂 | ✅ 2026-05-12 |
| P2 | 运数/劫数 Roll UI | ✅ 2026-05-12 |
| P3 | 遗念 overlay 重构（毛笔字书写） | ✅ 2026-05-12 |
| P4 | 重生流 cinematic + 坍缩渊死亡特殊处理 | ✅ 2026-05-12 |
| P5 | 终焉之言 + 饱和化测试 | ✅ 2026-05-12 |

---

## P0 — 状态机 + 骨架 + 通用死亡流 ✅ 2026-05-12

### 交付物

1. **`DeathCinematic` server component**（`server/src/death/death_cinematic.rs`）
   - 6 阶段枚举：`PreDeath` / `DeathMoment` / `Roll` / `InsightOverlay` / `Darkness` / `Rebirth`
   - 每阶段 duration_ticks（参数化按境界/死因）
   - server 权威推进 → emit `DeathCinematicS2c` packet

2. **`DeathCinematicRenderer` client 编排器**（`client/src/main/java/com/bong/client/death/DeathCinematicRenderer.java`）
   - 消费 `DeathCinematicS2c` → 按 phase 调度各 active plan 的效果：
     - **PreDeath**（60 tick / 3s）：`NearDeathOverlayPlanner` vignette 加深 + 心跳音效加速（调 audio-world-v1）+ camera FOV 微缩
     - **DeathMoment**（20 tick / 1s）：画框碎裂 + `DeathSoulDissipatePlayer` 魂散（调 vfx-wiring-v1）+ 灰化 overlay（调 hud-polish-v1）+ 死亡动画（调 player-animation-implementation-v1）+ 音效 `death_moment.json`
     - **Roll**（80 tick / 4s）：运数 roll UI（P2 实现）
     - **InsightOverlay**（120 tick / 6s）：遗念书写（P3 实现）
     - **Darkness**（40 tick / 2s）：全黑 + 静音
     - **Rebirth**（60 tick / 3s）：灵龛 fade-in + 虚弱灰雾 + 降境 narration

3. **画框碎裂效果**（`ScreenShatterEffect.java`）
   - death_moment 瞬间：截取当前帧 framebuffer → 分割为 12-16 块不规则碎片（Voronoi 剖分）
   - 碎片各自旋转 + 向外飞散（physics: gravity + random velocity，0.5s 全部消散）
   - 碎片飞散过程中背景渐黑
   - 纯 client 侧 DrawContext 实现（不需 shader mod）

4. **通用流程默认参数**
   - 总时长 ~19s（PreDeath 3s + Death 1s + Roll 4s + Insight 6s + Dark 2s + Rebirth 3s）
   - 低境（醒灵/引气）：Roll 简化（1s 快速掷签 → 不拖节奏），Insight 短（3 行遗念）
   - 高境（固元+）：Roll 正式（4s 完整仪式），Insight 长（10+ 行遗念）

### 验收抓手

- 测试：`server::death::tests::cinematic_phase_sequence` / `client::death::tests::screen_shatter_voronoi` / `client::death::tests::renderer_dispatches_by_phase`
- 手动：HP → 0 → 画框碎裂 → 全黑 → roll → 遗念 → 黑暗 → 灵龛重生

---

## P1 — 濒死三层崩解 ✅ 2026-05-12

### 交付物

1. **真元崩解**（HP < 20% 时第一层）
   - 真元条间歇闪烁红光（hud-polish-v1 已有状态条 flash → 叠加频率：HP 越低闪越快）
   - 身周真元逸散粒子（`BongSpriteParticle` `qi_aura` tint 按流派颜色，从身体向外飘散——"真元在流失"）
   - 逸散密度 = (1 - HP/HP_max) × 3

2. **经脉崩解**（HP < 10% 时第二层，叠加第一层）
   - 任意经脉 SEVERED → 对应肢体冒红色光线粒子（`BongLineParticle` 从经脉断点向外辐射）
   - 无 SEVERED 经脉时：随机经脉位置微红闪（暗示经脉在承压）
   - 与 `MiniBodyHudPlanner` 伤口剪影对齐（同一位置同一时机）

3. **体表崩解**（HP < 5% 时第三层，叠加前两层）
   - 任意 FRACTURE/SEVERED → 伤口位置溢血粒子（`BongSpriteParticle` `cloud256_dust` tint #5A0000 × 2 per 10 tick，从伤口向下滴落）
   - 全身体表微裂纹（`BongLineParticle` 随机短线 × 8 贴在 entity 表面，红色，lifetime 30 tick，每 60 tick 刷新——持续"龟裂"视觉）
   - 心跳音效最快模式（audio-world-v1 已有 → 通过 `AudioTriggerS2c` 加速 emit 频率）

4. **崩解→死亡衔接**
   - 3 层崩解持续到 HP = 0 → 所有崩解粒子瞬间冻结 0.3s（时间停顿感）→ 然后爆散 → 进入 DeathMoment 阶段

### 验收抓手

- 测试：`client::death::tests::qi_escape_density_by_hp` / `client::death::tests::meridian_glow_on_severed` / `client::death::tests::surface_crack_lines_refresh` / `client::death::tests::collapse_freeze_before_death`
- 手动：受伤 HP < 20% → 真元逸散 → HP < 10% → 经脉闪红 → HP < 5% → 体表裂纹+滴血 → HP 0 → 粒子冻结 → 爆散 → 死亡

---

## P2 — 运数/劫数 Roll UI ✅ 2026-05-12

### 交付物

1. **`DeathRollUI`**（`client/src/main/java/com/bong/client/death/DeathRollUI.java`）
   - DeathMoment 结束后 → Roll 阶段开始
   - 屏幕中央黑色背景上：
     - 顶部：运数/劫数 文字（"运数" = 正常死亡 / "劫数" = 渡劫死亡 / "天谴" = 天道干预死亡）
     - 中间：概率数字大字体 "65%"（此次存活概率）+ 下方小字 "（上次 85%）"
     - 动画：数字从 100% 滚动下降到实际概率值（1.5s 滚动 → 定格 0.5s）

2. **掷签动画**
   - 概率定格后 → 掷签：
     - 3 根竹签从上方落入画面 → 散落在黑色背景上
     - 按概率：
       - 成功（存活）：签面朝上显示"生"字 → 绿光微闪 → 1s 后进入 InsightOverlay
       - 失败（境界降落）：签面显示"落"字 → 红光微闪 → 降几阶段看概率 roll
       - 终焉（永久死亡）：签面显示"终"字 → 全签碎裂 → 进入终焉之言
   - 竹签 physics：gravity 0.5 + 落地 bounce 1 次 + friction 停止

3. **roll 过程中的玩家体验**
   - 全屏黑背景（仅 roll UI 可见——世界消失了）
   - 音效：`death_roll.json`（`minecraft:block.note_block.chime`(pitch 随机 0.8-1.2, volume 0.2) × 3 递进 → 最后一声定结果）
   - 结果出来后 1s 停顿 → 进入下一阶段

4. **高境界增强**
   - 固元+：roll UI 追加"天道注意力"条（天道关注你的死亡——高注意力 = 概率更低）
   - 化虚：roll 前追加 agent narration "天道在审视你的一生"（2s 额外停顿——更大压力）

### 验收抓手

- 测试：`client::death::tests::roll_probability_scrolls_to_actual` / `client::death::tests::bamboo_slip_physics` / `client::death::tests::roll_result_matches_server`
- 手动：死亡 → 黑屏 → "运数" + 概率滚动 65% → 掷签 → 签落 → "生" 绿光 / "落" 红光 / "终" 碎裂

---

## P3 — 遗念 overlay（毛笔字） ✅ 2026-05-12

### 交付物

1. **`InsightOverlayRenderer`**（`client/src/main/java/com/bong/client/death/InsightOverlayRenderer.java`）
   - Roll 结束后 → InsightOverlay 阶段
   - 全屏黑色微透（alpha 0.85）背景 + 遗念文字
   - **毛笔字逐笔书写动画**：
     - 文字从右上角开始，竖排书写（中文古籍排版方向）
     - 每字 0.3s 落墨（alpha 0→1 + 微放大 1.1→1.0）
     - 墨色 #C0B090（不是纯白/纯黑——旧纸色调）
     - 书写完一行后 0.5s 延迟 → 下一行

2. **遗念内容差异**
   - 低境（醒灵/引气）：3 行简短遗念（"此人灵脉微弱，死因：坠崖。骨币散落一地。"）
   - 中境（凝脉/固元）：6-8 行（追加经脉状态 / 修炼进度 / 杀了谁被谁杀 / 特殊 flag）
   - 高境（通灵/化虚）：10-15 行全屏遗念（追加天道评语 / 生平关键事件 / 修行路线总结）
   - 文本来自 `DeathInsight`（server death_lifecycle 已有生成逻辑）→ `DeathCinematicS2c.insight_text`

3. **遗念特殊格式**
   - 劫数期 roll 成功（侥幸存活）→ 追加一行红色："此次运数：35%。下次 20%。"——每死一次概率递减的冰冷提醒
   - 遗念中提到其他玩家名时 → 名字颜色与对方 realm 一致（暗示 identity 系统）
   - TSY 死亡 → 追加："秘境所得悉数散落。"（worldview §十六）

4. **voice-of-heaven 音效**
   - 遗念书写期间：低沉诵读音效 loop
   - `death_insight_voice.json`：`minecraft:ambient.cave`(pitch 0.2, volume 0.15) + `minecraft:entity.experience_orb.pickup`(pitch 0.3, volume 0.05, loop 4s)
   - 书写完成后音效 fade out 2s

### 验收抓手

- 测试：`client::death::tests::insight_calligraphy_line_by_line` / `client::death::tests::insight_line_count_by_realm` / `client::death::tests::insight_tsy_drop_warning`
- 手动：死亡 → roll → 遗念开始 → 竖排毛笔字逐字出现 → 低境 3 行 / 高境 15 行 → 特殊格式验证

---

## P4 — 重生流 + 坍缩渊死亡 ✅ 2026-05-12

### 交付物

1. **重生 cinematic**
   - Darkness 阶段：全黑 2s + 全静音（世界不存在了 0.5s → 然后远处灵龛微光逐渐出现）
   - Rebirth 阶段：
     - 灵龛位置 fade-in 3s（从全黑 → 正常亮度，world render 逐渐恢复）
     - 重生动画（player-animation-implementation-v1 `rebirth_wake`）：缓慢站起 + 环顾
     - 境界降一阶 HUD 通知：realm 名称闪红 1s → fade + toast "境界跌落 · [新境界]"
     - 3min 虚弱灰雾开始（`ExhaustedGreyOverlay` 复用 → alpha 0.15 持续 3min）
     - 音效：`rebirth_wake.json`（`minecraft:block.respawn_anchor.set_spawn`(pitch 0.6, volume 0.3) + `minecraft:ambient.cave`(pitch 0.5, volume 0.1)——灵龛共振 + 空洞）
     - agent narration："你还活着。代价已付。"

2. **坍缩渊内死亡特殊处理**
   - `TsyPresence` active → 死亡 cinematic 叠加：
     - DeathMoment：干尸化 VFX（玩家模型颜色 lerp → #8B7355 灰褐 1s + model scale 0.9 — 脱水干瘪）
     - 追加掉落可视化：所有 TSY 内获得物品从尸体位置弹射（3D 弹射弧 + 物品 floating tag "秘境所得"）
     - Roll 阶段追加文字："秘境的代价。"
     - Insight 追加："坍缩渊，概不赊欠。"
   - 灵龛重生位置 = 坍缩渊入口外（已由 server 处理）→ rebirth 阶段 fog 从 TSY dark fog 快速 lerp → 正常世界 fog

3. **多周目死亡差异**
   - 第一次死亡：完整 19s cinematic
   - 第 2-5 次：Roll 缩短 2s / Insight 加速（每字 0.15s）
   - 第 5+ 次：skip 到 Roll（不走 PreDeath/DeathMoment 完整流程——"你已经习惯了"）
   - 终焉（永久死亡）：不 skip，完整走终焉之言流程（P5）

### 验收抓手

- 测试：`client::death::tests::rebirth_fade_from_black` / `client::death::tests::tsy_mummification_visual` / `client::death::tests::death_count_shortens_cinematic`
- 手动：死亡 → 完整流程 → 灵龛重生 → 虚弱灰雾 → 再死 → 流程加速 → 坍缩渊死亡 → 干尸化 + 100% 掉落弹射

---

## P5 — 终焉之言 + 饱和化测试 ✅ 2026-05-12

### 交付物

1. **终焉之言 cinematic**（`FinalWordsRenderer.java`）
   - Roll 结果为"终"→ 角色永久死亡 → 特殊 cinematic：
     - 全屏黑色 → 毛笔字"终焉之言"大字居中 fade-in 2s
     - 生平卷归档：角色一生关键事件逐条列出（竖排书写，每条 0.5s）
       - "醒灵于初醒原·第三日"
       - "首杀 · 散修·无名"
       - "凝脉 · 灵泉湿地·第十九日"
       - "终焉 · [死因] · [位置] · [凶手]"
     - 列完后：所有文字化为金色粒子 → 卷起成卷轴 → 飞入天空消散（"天道归档了你的一生"）
     - 音效：`minecraft:music.credits`(pitch 0.3, volume 0.15, 5s)（宏大但克制）
   - 结束后 → 多周目系统接手（plan-multi-life-v1 已有）→ 新角色开始

2. **饱和化测试**
   - 6 境界 × 2 roll 结果（存/降/终）× 2 死因（普通/TSY）× 3 多周目阶段（首次/5次/终焉）= 72 组合
   - 关键路径：
     - 完整 19s cinematic 从头到尾连贯性
     - Roll UI 动画流畅 + 结果与 server 一致
     - 遗念书写 + voice 音效同步
     - 重生后虚弱灰雾 3min 确认
     - TSY 干尸化 + 掉落弹射

3. **多客户端同步**
   - 玩家 A 死亡 → 玩家 B 在旁观看：B 看到 A 的画框碎裂效果 + 魂散粒子 + 掉落物弹射
   - B 不看到 A 的 roll/遗念/黑暗（这些是 A 的私人 cinematic）

### 验收抓手

- 自动化：`scripts/death_cinematic_test.sh`（遍历关键组合 + 截图）
- 帧率：cinematic 全程 < 3ms GPU 额外开销
- 终焉之言：完整走一次永久死亡流程 → 生平卷归档 → 金粒子卷轴

---

## Finish Evidence

- **落地清单**：
  - P0 server 契约与状态机：`server/src/schema/death_cinematic.rs`、`server/src/death_lifecycle/cinematic.rs`、`server/src/schema/server_data.rs`，`DeathCinematicS2cV1` 挂入 `death_screen.cinematic`。
  - P0/P2/P3/P4/P5 client 编排：`client/src/main/java/com/bong/client/death/DeathCinematicState.java`、`DeathCinematicPayloadParser.java`、`DeathCinematicRenderer.java`、`DeathRollUI.java`、`InsightOverlayRenderer.java`、`FinalWordsRenderer.java`、`RebirthCinematicRenderer.java`，并由 `DeathScreen` 渲染 cinematic commands。
  - P1 client 视觉骨架：`NearDeathCollapsePlanner.java`、`ScreenShatterEffect.java` 覆盖濒死崩解密度、经脉闪红、体表裂纹、死亡前冻结和 16 片画框碎裂 command。
  - P4 坍缩渊/多周目参数：server `build_death_cinematic(...)` 写入 `zone_kind`、`tsy_death`、`death_number`、`rebirth_weakened_ticks`；client 根据 `tsyDeath`、`deathNumber`、`skipPredeath` 调整重生提示与阶段时长。
  - agent/schema 订阅：`agent/packages/schema/src/death-cinematic.ts`、`agent/packages/schema/src/channels.ts`、`agent/packages/tiandao/src/redis-ipc.ts`，新增 `bong:death_cinematic` 校验与 cross-system event buffer。
- **关键 commit**：
  - `da1d652cc` 2026-05-12 `feat(death-cinematic): 接入死亡电影化状态契约`
  - `4ec2ef8e4` 2026-05-12 `feat(client): 渲染死亡重生电影化流程`
  - `327c3c54f` 2026-05-12 `feat(agent): 订阅死亡电影化事件`
  - `6e5dcd739` 2026-05-12 `docs(plan-death-rebirth-cinematic-v1): finish evidence 并归档至 finished_plans/`
  - `108859ad7` 2026-05-12 `fix(death-cinematic): 补 CodeRabbit 边界测试与解析护栏`
- **测试结果**：
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：通过，`cargo test` 4568 passed。
  - `cd agent && npm run build && npm test -w @bong/schema && npm test -w @bong/tiandao`：通过，schema 383 passed，tiandao 362 passed。
  - `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`：通过，BUILD SUCCESSFUL。
  - CodeRabbit review follow-up：`cargo test death_lifecycle::cinematic`、`cargo test network::combat_bridge`、`./gradlew test --rerun-tasks --tests "com.bong.client.death.DeathCinematicTest" --tests "com.bong.client.combat.handler.CombatHandlersTest"`、`npm test -w @bong/schema -- schema.test.ts`、`npm test -w @bong/tiandao -- redis-ipc.test.ts` 均通过。
  - `git diff --check`：通过。
- **跨仓库核验**：
  - server：`DeathCinematicS2cV1`、`DeathCinematic`、`DeathCinematicPublished`、`CH_DEATH_CINEMATIC`、`DeathScreenS2cV1.cinematic`。
  - agent/schema：`DeathCinematicS2cV1`、`validateDeathCinematicS2cV1Contract`、`CHANNELS.DEATH_CINEMATIC`、`death-cinematic-s2c-v1.json`。
  - agent/tiandao：`RedisIpc` 订阅 `DEATH_CINEMATIC` 并记录 cross-system event。
  - client：`DeathCinematicPayloadParser`、`DeathCinematicState`、`DeathCinematicRenderer`、`DeathRollUI`、`InsightOverlayRenderer`、`FinalWordsRenderer`、`RebirthCinematicRenderer`。
- **遗留 / 后续**：
  - 无阻塞遗留；本 plan 已固定 server→agent→client cinematic contract、阶段机、HUD command 编排和回归测试。
  - 真实 framebuffer shader、独立音效 asset、player animation asset、亡者博物馆链接与观战模式仍由后续 visual/audio/library plans 扩展，不在本 PR 内扩大范围。
