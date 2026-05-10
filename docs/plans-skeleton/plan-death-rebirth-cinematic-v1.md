# Bong · plan-death-rebirth-cinematic-v1 · 骨架

死亡与重生电影化体验。当前 `DeathSoulDissipatePlayer` 已有点魂散粒子，`NearDeathOverlayPlanner` 有濒死 vignette——但死亡瞬间到重生在灵龛的完整视听流程是断裂的。本 plan 把死亡从"状态跳变"升级为**可感知的电影化事件**：命悬一线 → 死亡瞬间画框碎裂 → 遗念 overlay → 黑暗 → 灵龛重生。

**世界观锚点**：`worldview.md §十二` 死亡是学费 | 运数/劫数 roll 概率 visible | 遗念真实信息 | 终焉之言 | 重生惩罚（降境 + 真元 0 + 3min 虚弱）· `§四` 37 条血条复合崩溃 → 濒死时的各层崩解应可见 · `§十六` 坍缩渊内死亡额外规则（秘境所得 100% 掉落 + 干尸化）

**library 锚点**：`world-0001 天道口述残编`（死亡 narration 语调）· `peoples-0007 散修百态`（重生者的心理）

**前置依赖**：
- `plan-death-lifecycle-v1` ✅ → 死亡判定/结算/重生全链路
- `plan-lifespan-v1` ✅ → 寿元扣减
- `plan-multi-life-v1` ✅ → 多周目
- `plan-vfx-v1` ✅ → 屏幕级叠加
- `plan-audio-implementation-v1` 🆕 skeleton → 死亡/重生音效 recipe
- `plan-particle-system-v1` ✅ → DeathSoulDissipatePlayer 接入
- `plan-HUD-v1` ✅ → MiniBodyHudPlanner 伤口剪影

**反向被依赖**：
- `plan-tsy-raceout-v2` 🆕 skeleton → 坍缩渊内死亡特殊 cinematic（塌缩吞噬）

---

## 接入面 Checklist

- **进料**：`death_lifecycle::DeathEvent` / `death_lifecycle::RebirthEvent` / `death_lifecycle::NearDeathEvent` / `DeathSoulDissipatePlayer` / `NearDeathOverlayPlanner` / `cultivation::Cultivation` / `lifespan::Lifespan`
- **出料**：`DeathCinematic` 状态机（predeath→death_moment→insight_overlay→darkness→rebirth）+ 濒死三层崩解可视化（真元闪烁 + 经脉红光 + 体表伤口溢血粒子 → 依次触发）+ 运数/劫数 Roll UI（概率数字浮现 + 转盘/掷签动画）+ 遗念 overlay（毛笔字逐笔书写 + voice-of-heaven 音频）+ 终焉之言 cinematic（角色终结 solo roll 失败 → 全屏书法字 + 生平卷归档粒子）+ 重生流（灵龛 fade-in + 3min 虚弱灰雾 + 降境 narration）
- **跨仓库契约**：server `DeathCinematic` 阶段机 → client `DeathCinematicRenderer`

---

## §0 设计轴心

- [ ] **濒死可视**：HP < 10% 时已有的 NearDeathOverlay + 新增三层崩解粒子（真元/经脉/体表依次崩溃）
- [ ] **死亡瞬间 = 画框碎裂**：屏幕玻璃碎裂效果 + 魂散粒子 + 0.5s 冻结 + 进入遗念
- [ ] **遗念 = 真实信息 + 冷漠语调**：agent 生成遗念 overlay（毛笔字逐笔写 + voice-of-heaven 音频），不甜不暖
- [ ] **运数 roll = 紧张感**：概率数字显示 + 掷签/转盘动画（不是 slot machine，是"天意"的冷漠呈现）
- [ ] **重生 = 并非完全回归**：灵龛 fade-in + 虚弱灰雾 3min + 境界降一阶 narration

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `DeathCinematic` 状态机骨架（6 阶段）+ `DeathCinematicRenderer` 骨架 + 通用死亡→重生全流程（不区分运数/劫数/终焉）+ screen shatter effect（Minecraft `WindowFramebuffer` 截图 + 碎片粒子）+ DeathSoulDissipatePlayer 接入新状态机 | ⬜ |
| P1 | 濒死三层崩解可视化：真元池 → `qi_max 20%` 时真元条间歇闪烁红光 / 经脉 → 任意 SEVERED 时对应肢部冒红光粒子 / 体表 → 任意 FRACTURE/SEVERED 时伤口溢血粒子（与 MiniBodyHudPlanner 伤口剪影对齐） | ⬜ |
| P2 | 运数/劫数 Roll UI：死亡瞬间弹出概率数字（"此次运数：65%"→ 3s 倒数 → 掷签动画 [竹签散落/铜钱旋转] → 结果 [成: 绿光闪烁 / 败: 红光碎裂]）+ 终焉之言 cinematic（solo roll 失败 → 全屏毛笔字书写 + 生平卷归档金色粒子 + voice 旁白） | ⬜ |
| P3 | 遗念 overlay 重构：agent 生成遗念文本 → client 毛笔字逐笔书写动画（从右上角开始，逐笔落墨）+ voice-of-heaven 冷漠诵读（与 plan-audio-implementation-v1 voice 音效联动）+ 境界差异（醒灵 3 行 / 化虚全屏 15 行）+ 特殊遗念（劫数期 roll 成功 → "此次运数：35%。下次 20%。"） | ⬜ |
| P4 | 重生流：灵龛位置 fade-in（3s 从黑到亮）+ 境界降一阶 HUD 闪烁 + 3min 虚弱灰雾不散（ExhaustedGreyOverlay 复用）+ 降境 narration 弹出 + 坍缩渊内死亡特殊处理（秘境所得 100% 掉落可视化 + 干尸化粒子） | ⬜ |
| P5 | 饱和化测试：全 6 境界 × 2 结果（运数/劫数）× 终焉之言 × 坍缩渊死亡× 多客户端围观 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`DeathCinematic` 状态机 / `DeathCinematicRenderer` / 濒死三层崩解 / Roll UI / 遗念 overlay / 终焉之言 / 重生流 / 坍缩渊特殊死亡
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：亡者博物馆链接（library-web 角色生平卷直接链接）
