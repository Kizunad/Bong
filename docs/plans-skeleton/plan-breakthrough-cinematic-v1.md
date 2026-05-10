# Bong · plan-breakthrough-cinematic-v1 · 骨架

突破/修炼电影化奇观。当前 `BreakthroughPillarPlayer` 已有点柱粒子，但突破全程缺乏多阶段视听叙事——玩家静坐 3 分钟突破时只有一条 narration 弹出。本 plan 把每个突破变成**可感知的灵气事件**：灵压骤变 → 经脉光路显形 → 天地共鸣 → 全服可见的异象。

**世界观锚点**：`worldview.md §三` 六境界突破条件（静坐 + 灵气环境 + 局部循环 → 凝核 → 共鸣 → 渡劫）· `§八` 天道对突破的态度（低境怜悯 / 高境敌视）· `§十七` 突破与季节共振（夏季雷劫可期 / 冬季爆发不足 / 汐转高风险）· `§四` 境界越一级是"可咬一口"——突破瞬间是脆弱的

**library 锚点**：`cultivation-0001 六境要录`（突破体征描述）· `cultivation-0006 经脉浅述`（经脉光路走向）

**前置依赖**：
- `plan-cultivation-v1` ✅ → BreakthroughRequest / Realm 转换事件
- `plan-particle-system-v1` ✅ → BongLineParticle / BongRibbonParticle / BongGroundDecalParticle 渲染基类
- `plan-vfx-v1` ✅ → 屏幕级叠加（HUD 叠色 / 镜头 FOV / 抖屏）
- `plan-audio-implementation-v1` 🆕 skeleton → 突破 pulse / 共鸣嗡鸣 recipe
- `plan-player-animation-implementation-v1` 🆕 skeleton → 突破姿态动画
- `plan-HUD-v1` ✅ → TribulationBroadcastHudPlanner（全服广播复用）
- `plan-jiezeq-v1` 🆕 active → 突破与季节共振 hook

**反向被依赖**：
- `plan-tribulation-v2` 🆕 active → 渡虚劫可叠加突破 cinematic（化虚渡劫是双重 spectacle）

---

## 接入面 Checklist

- **进料**：`cultivation::BreakthroughRequest` event / `cultivation::MeridianSystem` / `cultivation::Cultivation { qi_current, qi_max, realm }` / `BreakthroughPillarPlayer`（已有，进料复用）/ `RealmVisionState`（zone 灵气可视化）
- **出料**：`BreakthroughCinematic` component（跟踪阶段: prelude→charge→catalyze→apex→aftermath）+ 每境专用粒子（醒灵→引气 灵气涡旋 / 引气→凝脉 经脉光路 / 凝脉→固元 凝核光球 / 固元→通灵 天地光柱 / 通灵→化虚 全服异象）+ 屏幕效果序列（FOV 微缩 → 白闪 → 染色叠层 → 渐清）+ 音效 sequence（心跳渐快 → 洪钟 → 长余韵）
- **跨仓库契约**：server `BreakthroughCinematic` 阶段机（权威推进）→ client `BreakthroughSpectacleRenderer`（纯表演层）

---

## §0 设计轴心

- [ ] **每境不同视觉**：不同境界突破的粒子/光效/音效/时长各不相同（醒灵→引气 30s 轻盈 / 通灵→化虚 3min 天地异变）
- [ ] **全服可见异象**：凝脉+ 突破在突破点生成天空光柱/灵压波动，5km 内可见
- [ ] **可打断 = 高张力**：突破中其他玩家/NPC 可攻击打断 → 突破方在 cinematic 最脆弱时被偷袭的叙事张力
- [ ] **agent narration 与 cinematic 双轨同步**：天道旁白 + 视觉奇观同时推进

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `BreakthroughCinematic` 状态机（5 阶段: prelude/charge/catalyze/apex/aftermath）+ `BreakthroughSpectacleRenderer` 骨架 + 醒灵→引气 完整 cinematic（30s，粒子涡旋 + 白闪 + 心跳音效）+ `BreakthroughPillarPlayer` 重构接入新状态机 | ⬜ |
| P1 | 引气→凝脉 cinematic（经脉光路沿 12 正经粒子流 + 局部循环光带）+ 凝脉→固元 cinematic（真元凝核球体从半透明渐变成固态 + 灵眼坐标闪光）+ 每境专用 screen effect 序列（FOV/screen tint/shake 参数化） | ⬜ |
| P2 | 固元→通灵 cinematic（天地光柱 + 全服可见 5km 异象 + agent narration 同步 "某处有人在通灵"）+ 通灵→化虚 cinematic（渡虚劫前夜 prelude + 天道注视黑云 + 全服广播 10km 异象） | ⬜ |
| P3 | 突破打断视觉（cinematic 中 hit → 粒子爆散 + 红色 screen flash + "突破失败"叠字）+ 突破成功庆祝（apex→aftermath 金色粒子雨 + HUD 新境界闪烁）/ 突破失败回落（境界未变的败兴粒子 + agent 嘲讽 narration） | ⬜ |
| P4 | 与季节系统联动（夏季突破 cinematic 加雷光粒子 / 冬季突破加冰晶粒子 / 汐转突破加紊乱扭曲效果）+ 与玩家动画联动（突破 cinematic 中 PlayerAnimator 自动切突破姿态） | ⬜ |
| P5 | 饱和化测试：5 境突破各 10 次 × 2 结果（成功/失败）× 3 季节 × 多客户端围观 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`BreakthroughCinematic` component / `BreakthroughSpectacleRenderer` / 5 境粒子+光效 / screen effect 序列 / 音效 sequence / agent narration 联动
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：跨玩家突破交互视觉（两人同时同地突破的叠加效果）
