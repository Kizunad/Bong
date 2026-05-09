# Bong · plan-dugu-v2 · 骨架

毒蛊功法五招完整包：动画 / 特效 / 音效 / 伤害 / 真元消耗 / 反噬 / 暴露身份 / 客户端 UI 全流程。承接 `plan-dugu-v1` ✅ finished（PR #126 commit 44a6ff9b 归档，shoot_needle P0 已实装）—— v2 引入**脏真元物理**（ρ=0.05 几乎无异体排斥）+ **永久阈值分三档**（低三段仅 HP/qi / 固元短期 / 通灵+ 永久 qi_max 衰减）+ **自蕴**（自身经脉养成毒源）+ **暴露概率系统** + **阴诡色形貌异化**，五招完整规格（蚀针 / 自蕴 / 侵染 / 神识遮蔽 / 倒蚀），无境界 gate 只有威力门坎。**严守 worldview §五 + §六 正典**，不引入"蛊母 / 蛊虫 / 虫卵"等偏离虫子叙事 ——「蛊」在本 plan 内仅作汉字"诡毒"意，不出现具体虫子。

**世界观锚点**：`worldview.md §三:368 越级原则物理事实`（永久阈值分级依据）· `§四:506 末土后招原则`（神识遮蔽 / 伪示）· `§五:421-427 毒蛊核心定义`（飞针+毒草+脏真元+寄生比喻）· `§五:520-535 毒 vs 毒蛊关键边界`（暴露身份 + 全服追杀社会后果）· `§六:618-621 阴诡色长期沉淀`（自身经脉慢性侵蚀）· `§六:625 染色形成 ~10h 主色`（自蕴时间下界）· `§十一:947-970 NPC 反应分级 + 毒蛊师 baseline -50`· `§十一:967 高境 NPC 神识识破`· `§三:78 化虚天道针对 + §十一 灵物密度阈值`（倒蚀化虚级触发绝壁劫）· `§K narration 沉默`

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §缚论`（脏真元封入物质化身的物理推导）· `peoples-0007 散修百态 假死路`（毒蛊师社会生存指南，与神识遮蔽机制同源）

**前置依赖**：

- `plan-qi-physics-v1` P1 ship → 脏真元 ρ=0.05 走 `qi_physics::collision::qi_collision` 算子
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派异体排斥 ρ 矩阵实装（毒蛊 ρ=0.05）
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / Casting / cooldown 直接复用
- `plan-multi-style-v1` ✅ → PracticeLog vector 接入阴诡色累积
- `plan-identity-v1` ⏳ active → DuguRevealedEvent consumer，社会反应分级（NPC 信誉度翻脸）
- `plan-botany-v2` ✅ → 自蕴专属毒草（毒草特性已 17 物种内，留 botany 配合补 dugu 专属几种）
- `plan-HUD-v1` ✅ + `plan-input-binding-v1` ✅

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 招的 ρ/W/D 数值进矩阵（毒蛊 ρ=0.05 / D=15）
- `plan-tribulation-v1` ⏳ → 化虚倒蚀触发绝壁劫
- `plan-narrative-political-v1` ✅ active → 毒蛊师暴露身份后江湖传闻型政治叙事

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, contamination, qi_color }` / `qi_physics::ledger::QiTransfer` / `qi_physics::collision::qi_collision`（ρ=0.05 脏真元注入）/ `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm` / `IdentityProfile`(plan-identity-v1) / `botany::PlantRegistry`（自蕴毒草）
- **出料**：5 招 `DuguSkillId` enum 注册到 SkillRegistry / `EclipseNeedleEvent` 🆕（蚀针命中 + 注入剂量 + 持续效果分级）/ `SelfCureProgressEvent` 🆕（自蕴累积阴诡色）/ `PenetrateChainEvent` 🆕（侵染联级触发）/ `ShroudActivatedEvent` 🆕（神识遮蔽开关）/ `ReverseTriggeredEvent` 🆕（化虚倒蚀引爆）/ `DuguRevealedEvent`（plan-dugu-v1 stub 已存，由 plan-identity-v1 consume）/ `TaintMark` component 写入受害者 / `PermanentQiMaxDecay` component（通灵+ 受害者持久化）
- **共享类型**：`StyleAttack` trait（qi_physics::traits）/ `EnvField` 扩展加 `dugu_taint_residue`（流派识别痕迹）+ `self_cure_aura`（自蕴气息）+ `reverse_aftermath_cloud`（倒蚀余响）
- **跨仓库契约**：
  - server: `combat::dugu_v2::*` 主实装 / `schema::dugu_v2`
  - agent: `tiandao::dugu_v2_runtime`（5 招 narration + 暴露身份江湖传闻 + 倒蚀化虚级绝壁劫预兆 + 受害者 qi_max 永久衰减心理叙事 + 自蕴形貌异化叙事）
  - client: 4 动画 + 3 粒子 + 3 音效 recipe + 4 HUD 组件
- **worldview 锚点**：见头部
- **qi_physics 锚点**：脏真元 ρ=0.05 走 `qi_physics::constants::DUGU_RHO`(待 patch P3 加) + `qi_physics::collision::qi_collision`；侵染联级走 `qi_physics::field::propagate_to_tainted_targets` 🆕（patch P3 加新算子）；倒蚀引爆走 `qi_physics::field::reverse_burst_all_marks(zone)` 🆕（patch P3 加）；**禁止 plan 内自己写脏真元注入/联级/引爆公式**

---

## §0 设计轴心

- [ ] **脏真元物理（区别于普通真元）**：worldview §五:425 寄生虫机制 + §六:618 阴诡色物理化身。
  ```
  普通真元 ρ ∈ [0.15, 0.65]    异体排斥率（攻方真元被宿主免疫）
  脏真元 ρ = 0.05               几乎无异体排斥（伪装成宿主真元混入经脉）

  脏真元代价（受害者侧）：
    醒灵-凝脉受害：仅 HP / qi 即时扣除（低境免疫系统中和脏真元）
    固元受害：qi_max 短期 -2-5% / 24h 自然代谢恢复
    通灵+受害：qi_max 永久 -0.05-0.1%/min（worldview §五:425 寄生破坏）

  中和方式：
    自排：受害者花 20+ qi 强行排出（自伤）
    专属解蛊药：仅毒蛊师能炼（worldview §五:528）
    失败永久废经脉：解蛊药使用失败 → 受影响经脉 SEVERED
  ```

- [ ] **永久阈值分三档（worldview §三:368 越级原则物理化身）**：
  | 受害者境界 | 蚀针后果 | 物理依据 |
  |---|---|---|
  | 醒灵 / 引气 / 凝脉 | 仅即时 HP + qi 扣，无任何持续 | 低境界宿主真元免疫系统中和脏真元 |
  | 固元 | qi_max 短期 -2-5% / 24h 自然代谢恢复 | 中境过渡态——可恢复 |
  | 通灵 / 化虚 | qi_max **永久** -0.05-0.1%/min（直到中和或受害者死） | worldview §五:425 寄生破坏物理 |

  设计意图：**只有通灵以上的毒蛊师能造成永久标记**——低境毒蛊师只能放血追猎，是 worldview §三 越级原则的真物理化身。

- [ ] **自蕴（自身经脉养成毒源）**：worldview §六:621「自身经脉慢性侵蚀，需持续养」物理化身。**不是养虫**，是**养自己**：
  ```
  服食毒草煎汤 + 自身真元淬炼 → 阴诡色累积
  阴诡色 % = 全部蛊毒招式威力乘数（×1.0 → ×3.0）

  累积曲线（边际递减——新手手感快、高手瓶颈慢，总累计 ~100h 到 90%）：
    daily_gain = 1.5%/h × (1 - current%/90)² × hours_meditated_today
    每日服食上限 6h（防挂机刷）

  典型时间表（按累计 in-game 服食时间）：
    0  → 5%  ：5h    （醒灵手感期，每小时 ~1%）
    5  → 10% ：12h   （+7h）
    10 → 20% ：25h   （+13h）
    20 → 30% ：40h   （+15h）
    30 → 60% ：70h   （+30h）
    60 → 90% ：100h  （+30h，化虚级瓶颈，每小时 ~0.5%）
    >  90%   ：渐近曲线，理论 ~95% 需 200h+
  ```
  **境界本身不影响累积速度**（worldview §六:625 染色物理基础对所有境界一致 + §五:537 流派由组合涌现无门禁）——醒灵也能慢慢自蕴成毒源，只是低境毒源的招式威力（蚀针 / 倒蚀）受 §0 永久阈值分级限制（醒灵阴诡色 90% 蚀针对低境受害者仍只能扣 HP/qi）。
  
  代价：阴诡色不可洗（worldview §六:631 普通可洗，但毒蛊师自蕴产生的是**永久结构改变**）。形貌异化 ≥ 60% 触发 IdentityProfile 自动写入 `dugu_self_revealed`——你不主动出招也会被高境 NPC 识破

- [ ] **暴露概率系统（区别于其他流派）**：worldview §五:530-535 毒蛊师暴露=全服追杀。每次主动招式（蚀针/侵染/倒蚀/伪示失败）触发暴露 roll：
  ```
  P(reveal) = base_rate × (1 - 神识遮蔽强度) × distance_factor × victim_realm_factor

    base_rate    = 醒灵 5% / 引气 4% / 凝脉 3% / 固元 2% / 通灵 1% / 半步化虚 0.5% / 化虚 0.2%
    神识遮蔽强度 = ④招式参数（0.0-0.95）
    distance_factor = 1.0 (≤5格) / 0.7 (5-15格) / 0.4 (15+格)
    victim_realm_factor = 受害者境界 ≥ 固元 ×3（worldview §十一:967 高境 NPC 神识识破）
  ```
  被识破 → emit `DuguRevealedEvent { caster, victim, location }` → plan-identity-v1 写入 LifeRecord，IdentityProfile.npc_reaction baseline -50（worldview §十一:962-970 毒蛊师社会默认）

- [ ] **流派识别痕迹（毒蛊专属 EnvField 边界）**：worldview §五:537 流派由组合涌现的物理化身。三种形态：
  - **脏真元残留**（受害者侧）：被蚀针中过的实体 spirit_quality 标记 `dugu_tainted = { caster_id, intensity, since_tick }`，高境玩家 inspect 可见，低境隐匿
  - **自蕴气息**（毒蛊师周身 5 格 EnvField）：阴诡色 ≥ 30% 后，自身周围常驻雾气，凡器在手抖动，植物枯萎加速 ×1.5
  - **倒蚀余响**（化虚倒蚀触发后受害者位置）：暗绿蛊云覆盖 30s，半径 10 格内 zone qi 状态被覆盖（其他玩家可视，是身份暴露的物理风险源）
  - **守恒**：脏真元最终散回受害者所在 zone qi 静态值（worldview §二 真元极易挥发末法分解），但过渡态对受害者经脉造成永久结构改变（这是毒蛊不同于涡流"99% 紊流自然耗散"的关键 —— 化学侵蚀不可逆）

- [ ] **无境界 gate，只有威力门坎**（worldview §五:537）：5 招都允许任何境界 cast，三层物理自然惩罚：
  - qi_current 不足 → server `Rejected{ QiInsufficient }`
  - 永久阈值未达（醒灵-凝脉无永久效果）→ HUD「蚀针擦皮」/「蛊毒入肉」表示触发但威力低
  - 倒蚀通灵以下 → 「指无应」（无已种入永久标记可引爆）

- [ ] **化虚倒蚀触发绝壁劫**（worldview §三:78 + §十一）：化虚级 zone 量级倒蚀 = 大规模杀戮 → 触发天道注视急剧累积 → 30s 内必降"绝壁劫"。化虚毒蛊师每次清算 = 引天道下来 = 极端孤注，跟涡心化虚一致格调

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：5 招数值表锁定 + 永久阈值分三档对齐 + 自蕴时间曲线锚定 + 暴露概率公式 + §5 七决策门收口 + qi_physics 接入面定稿 + 与 plan-style-balance-v1 ρ/W 矩阵对齐 + EnvField 三种痕迹 design | 数值矩阵 + 物理公式落 plan §2 / qi_physics::env 痕迹 design 对齐 |
| **P1** ⬜ | server `combat::dugu_v2::*` 5 招 logic + 脏真元物理 + 永久阈值分级 + 自蕴累积 + 暴露概率 + EnvField 痕迹写入 + qi_physics 算子调用 + ≥120 单测（每招 ≥15 测覆盖 happy/边界/暴露/永久阈值跨档/守恒断言）| `cargo test combat::dugu_v2` 全过 / 守恒断言（脏真元 99% 散回 zone）/ 永久阈值分档测试每档完整 / `grep -rcE '#\[test\]' server/src/combat/dugu_v2/` ≥ 120 |
| **P2** ⬜ | client 4 动画（蚀针掷出 / 自蕴服毒 / 神识遮蔽姿态 / 倒蚀远指）+ 3 粒子（DUGU_DARK_GREEN_MIST / DUGU_TAINT_PULSE / DUGU_REVERSE_BURST）+ 4 HUD 组件（DuguTaintWarningHud / DuguTaintIndicator / RevealRiskHud / SelfCureProgressHud） | render_animation.py headless 验证通过 / WSLg 实跑 5 招视觉确认 / HUD 4 组件渲染无闪烁 |
| **P3** ⬜ | 3 音效 recipe（dugu_needle_hiss / dugu_self_cure_drink / dugu_curse_cackle）+ agent 5 招 narration template + 暴露身份江湖传闻型叙事（plan-narrative-political-v1 联调）+ 受害者 qi_max 永久衰减心理叙事 + 自蕴形貌异化叙事 + 化虚倒蚀绝壁劫预兆 | narration-eval ✅ 5 招 + 暴露 + 永久衰减心理 + 化虚级清算预兆 全过古意检测 |
| **P4** ⬜ | PVP telemetry 校准 / 缜密色 hook 改成阴诡色 hook（PracticeLog → QiColor 阴诡色累积演化）/ 坍缩渊负灵域内毒蛊行为（毒蛊师在坍缩渊内 不能 自蕴 因为环境真元极薄，需 plan-tsy-zone-v1 配合）/ 化虚绝壁劫触发链（plan-tribulation-v1）+ identity 暴露后社会追杀联调（plan-narrative-political-v1）| 7 流派 4×3 攻防对位毒蛊行通过 / 阴诡色长期累积演化测试 / 暴露后江湖传闻 narration 实测 |

**P0 决策门**：完成前 §5 七问题必须有答案，否则五招实装方向分裂。

---

## §2 五招完整规格

### ① 蚀针 — 飞针注入脏真元

**用途**：远程飞针（worldview §五:421 极其微小但穿透力极强）注入脏真元。低境扣 HP/qi，高境造成 qi_max 永久衰减。专长**漫长追猎战**——给猎物放血。

| 受害者境界 | 即时 HP/qi 扣 | 持续效果 | HUD（受害者侧）|
|---|---|---|---|
| 醒灵 | HP -2 / qi -3 | — | 蚀针擦皮 |
| 引气 | HP -5 / qi -8 | — | 蚀针入肉 |
| 凝脉 | HP -10 / qi -15 | — | 蚀针深刺 |
| 固元 | HP -15 / qi -25 | qi_max -2% / 24h 恢复 | 蛊毒入脉 |
| 通灵 | HP -20 / qi -40 | qi_max **永久** -0.05%/min | 蛊毒入髓 |
| 半步化虚 | HP -25 / qi -60 | qi_max 永久 -0.08%/min | 蛊毒蚀骨 |
| 化虚 | HP -40 / qi -100 | qi_max 永久 -0.1%/min | 蛊毒入魂 |

**caster qi 消耗**：8（一次性）+ 5（每命中追加，因脏真元淬炼成本）
**冷却**：3s
**自蕴加成**：上述持续效果 × (1 + 阴诡色 % × 2)（阴诡色 60% → ×2.2）
**worldview 锚**：§五:421-427 全段

### ② 自蕴 — 长期养自身经脉成毒源

**用途**：服毒草煎汤 + 自身真元淬炼 → **自身经脉变成毒源**。所有蛊毒招式威力 = 阴诡色 % 的乘数。这是毒蛊师的"修炼通路"，跟 cultivation 静坐冲击经脉**并行**而非替代。

| 阴诡色累积 % | 累计服食时间 | 自蕴效果 | 形貌外观 | inspect 可见 |
|---|---|---|---|---|
| 0%  | —    | 蛊毒招式 ×1.0 | 普通修士 | 无标识 |
| 5%  | 5h   | ×1.1 | 口齿青黑 | 仅化虚级神识可见 |
| 10% | 12h  | ×1.3 | 口齿明显泛黑 | 通灵+ 神识可见 |
| 20% | 25h  | ×1.5 | 皮色蜡黄 | 通灵+ 神识可见 |
| 30% | 40h  | ×1.7 | 鼻周泛黑 + 自蕴气息 3 格 | 固元+ inspect 可见 |
| 60% | 70h  | ×2.2 | 自蕴气息 5 格 + 凡器在手抖动 | **任何境界 inspect 可见 + 自动 dugu_self_revealed** |
| 90% | 100h | ×3.0（封顶）| 形貌完全异化，不可逆，无法以普通修士身份示人 | 同上 |

**机制**：每日服食 1 次毒草煎汤（qi 10 + 1 株 dugu 类毒草，每小时累积一次） → 触发当日累积。累积公式：`daily_gain = 1.5%/h × (1 - current%/90)² × hours_today`，每日服食上限 6h（防挂机刷，剩余时间不累积）。需在自家或安全点服食（被打断中止当日累积）。**境界不影响累积速度**——醒灵也能慢慢成毒源，只是招式威力受 §0 永久阈值分级限制。

**专属毒草**（plan-botany-v2 ✅ 已 17 物种 + 留几种 dugu 专属补，比如"赤髓草"/"夜哭蛇腹叶"等，具体留 botany 配合）

**代价**：阴诡色**永久不可洗**（区别于 worldview §六:631 普通染色可洗）。形貌异化 ≥ 60% → IdentityProfile 自动写 `dugu_self_revealed` 触发社会反应

**worldview 锚**：§六:618-621 阴诡色 + §六:625 染色 ~10h 主色（毒蛊属重度专精，需 30-100h）+ §五:533 毒蛊师社会代价

### ③ 侵染 — 二次注入触发联级

**用途**：已经被蚀针中过的目标，第二次蚀针 → 触发**已植入脏真元反应**，效果 ×N。**故意放过第一次没死的猎物**让他逃 → 7 天后第二次出现 → 一招速杀。

| 施法者境界 | 联级倍率 | 持续效果增幅 | 多目标半径 |
|---|---|---|---|
| 醒灵 | ×1.5 即时 | — | 单目标 |
| 引气 | ×1.8 即时 | — | 单目标 |
| 凝脉 | ×2.0 即时 | — | 单目标 |
| 固元 | ×2.5 + qi_max 短期 -3% / 24h | — | 单目标 |
| 通灵 | ×3.0 + qi_max 永久 -0.1%/min | 持续累加 | 单目标 |
| 半步化虚 | ×4.0 + 永久 -0.15%/min + 周围 5 格已植入蛊全部联级 | 多目标 | 5 格 |
| 化虚 | ×5.0 + 永久 -0.2%/min + 整 zone 联级 | zone 量级 | zone |

**caster qi 消耗**：12 + 8（每次联级追加）
**冷却**：8s
**触发条件**：目标必须已被蚀针中过（持有 `TaintMark` component）
**worldview 锚**：§五:425 寄生虫机制持续破坏 + §五:521 押注一招杀掉

### ④ 神识遮蔽 — 身份隐藏 + 伪示

**用途**：worldview §五:520-527 毒蛊师**主动遮蔽神识 + 伪造 qi_color 向量**，inspect 看到的是伪造的 `QiColor { main, secondary, is_chaotic, is_hunyuan }`。对手根据 qi_color 推断流派准备对策——看到 Heavy 以为体修贴脸、看到 Solid 以为截脉准备反弹，实际是毒蛊阴诡色。

| 施法者境界 | 神识遮蔽强度 | 可伪造 qi_color | 伪示持续 |
|---|---|---|---|
| 醒灵 | 0.20（暴露率仅降 20%）| 仅清空 secondary + is_chaotic（"普通修士"）| 1min |
| 引气 | 0.30 | + 可将 main 设为 Heavy | 3min |
| 凝脉 | 0.50 | + Solid / Sharp | 5min |
| 固元 | 0.70 | + 任意单色 main（10 色自选）| 10min |
| 通灵 | 0.85 | + 可设 secondary（双色向量伪造）| 30min |
| 半步化虚 | 0.92 | 全向量自由伪造 + is_hunyuan 可伪 | 1h |
| 化虚 | 0.95 | 全向量 + 可压低被 inspect 时的境界读数（降 2 阶）| 永久（直到主动关闭） |

**caster qi 消耗**：5（启动）+ 0.5/s（维持）
**机制**：开启后 `QiColor` 的 inspect 读数被临时 override（写入 `ShroudActive.fake_qi_color: QiColor`），下次主动招式暴露概率 × (1 - 强度)。**真实 qi_color 不变**——仅 inspect / 神识感知看到的是假值，战斗结算仍走真实 qi_color。被高境 NPC（worldview §十一:967）以 victim_realm_factor ×3 概率仍可识破（识破 = 同时暴露真实 qi_color + 阴诡色）
**worldview 锚**：§五:520-527 全段 + §四:506 末土后招原则

### ⑤ 倒蚀 — 化虚级远程引爆已种入毒（专属）

**用途**：远程一指 → 引爆**所有已种入敌人体内的脏真元永久标记**（多个玩家身上同时触发）→ 多经脉同时撕裂。**化虚级专属**——通灵以下试 = HUD「指无应」（无已种入永久标记可引爆，因为低境施法者本身造不出永久标记）。

| 施法者境界 | 引爆机制 | 多目标 | 反噬 |
|---|---|---|---|
| 醒灵-通灵 | — | — | HUD「指无应」 |
| 半步化虚 | 单目标 | 1 | 阴诡色 +5%（永久不可洗）|
| 化虚 | 整 zone 内所有已植入永久标记 | zone | 阴诡色 +5% × 引爆数 / 30s 内必降"绝壁劫" |

**caster qi 消耗**：50（启动）+ 30 × 引爆数
**冷却**：维持上限耗尽后 60s（化虚级孤注）
**叙事意象**：化虚毒蛊师隐居山林，每月一次"清算日"——所有曾被他蚀针扎过的人在同一秒经脉爆裂死亡。worldview §五:421 "极适合漫长追踪战"的极限化
**反噬**：阴诡色 +5% × 每个引爆目标，永久累加。引爆 ≥ 5 目标 → 形貌异化必至 90%+ → dugu_self_revealed
**绝壁劫触发**：worldview §三:78 + §十一 灵物密度阈值。化虚级倒蚀 = zone 内灵气大规模异常 → 30s 内必降"绝壁劫"（强度 ×1.5，无法过）。这是化虚毒蛊师最孤注的一击
**worldview 锚**：§五:521-526 押注一招的极限化 + §三:78 化虚天道针对

---

## §3 数据契约

```
server/src/combat/dugu_v2/
├── mod.rs              — Plugin 注册 + re-export + register_skills(&mut SkillRegistry)
├── skills.rs           — DuguSkillId enum (Eclipse/SelfCure/Penetrate/Shroud/Reverse)
│                        + 5 resolve_fn (cast_eclipse / cast_self_cure /
│                                       cast_penetrate / cast_shroud / cast_reverse)
├── state.rs            — DuguState component (阴诡色 %, 形貌异化 %,
│                                              dugu_self_revealed flag)
│                        + TaintMark component (受害者侧, caster_id +
│                                              intensity + since_tick + permanent_decay_rate)
│                        + ShroudActive component (神识遮蔽 active state +
│                                                 强度 + 伪造 qi_color + 持续 ticks)
│                        + SelfCureSession component (caster 服食 progress)
├── tick.rs             — taint_decay_tick (固元短期 24h 恢复) +
│                        permanent_qi_max_decay_tick (通灵+ 持续衰减) +
│                        shroud_maintain_tick + self_cure_progress_tick
├── physics.rs          — 脏真元注入 (调 qi_physics::collision::qi_collision ρ=0.05) +
│                        永久阈值分级判定 (低3段/固元/通灵+) +
│                        自蕴阴诡色累积公式 (递减式) +
│                        侵染联级判定 (扫描 TaintMark) +
│                        倒蚀引爆 (调 qi_physics::field::reverse_burst_all_marks)
├── reveal.rs           — 暴露概率计算 (base × (1-shroud) × dist × realm) +
│                        DuguRevealedEvent 触发 + IdentityProfile 写入
└── events.rs           — EclipseNeedleEvent / SelfCureProgressEvent /
                          PenetrateChainEvent / ShroudActivatedEvent /
                          ReverseTriggeredEvent / DuguRevealedEvent (扩展 v1 stub) /
                          PermanentQiMaxDecayApplied / DuguSelfRevealedEvent

server/src/schema/dugu_v2.rs  — IPC schema 5 招 + TaintMark + 暴露事件 payload

agent/packages/schema/src/dugu_v2.ts  — TypeBox 对齐
agent/packages/tiandao/src/dugu_v2_runtime.ts  — 5 招 narration +
                                                 暴露身份江湖传闻型叙事 +
                                                 受害者 qi_max 永久衰减心理叙事 +
                                                 自蕴形貌异化叙事 +
                                                 化虚倒蚀绝壁劫预兆

client/src/main/java/.../combat/dugu/v2/
├── DuguV2AnimationPlayer.java         — 4 动画播放
├── DuguDarkGreenMistParticle.java     — 阴诡色暗绿雾粒子
├── DuguTaintPulseParticle.java        — 脏真元在受害者身上的脉动可视
├── DuguReverseBurstParticle.java      — 倒蚀引爆瞬间爆开
├── DuguTaintWarningHud.java           — 自身被蚀针中后角落红点 + qi_max 衰减提示
├── DuguTaintIndicator.java            — 自身阴诡色 % 显示
├── RevealRiskHud.java                 — 当前招式暴露概率显示
└── SelfCureProgressHud.java           — 自蕴累积进度（每日服食次数 + 阴诡色目标）

client/src/main/resources/assets/bong/
├── player_animation/dugu_needle_throw.json
├── player_animation/dugu_self_cure_pose.json
├── player_animation/dugu_shroud_activate.json
├── player_animation/dugu_pointing_curse.json (倒蚀化虚级远指)
└── audio_recipes/dugu_needle_hiss.json + dugu_self_cure_drink.json + dugu_curse_cackle.json
```

**SkillRegistry 注册**：

```rust
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("dugu.eclipse",    cast_eclipse);    // 蚀针
    registry.register("dugu.self_cure",  cast_self_cure);  // 自蕴
    registry.register("dugu.penetrate",  cast_penetrate);  // 侵染
    registry.register("dugu.shroud",     cast_shroud);     // 神识遮蔽
    registry.register("dugu.reverse",    cast_reverse);    // 倒蚀
}
```

**PracticeLog 接入**（每招触发后）：

```rust
emit SkillXpGain {
    char: caster,
    skill: SkillId::Dugu,
    amount: per_skill_amount(skill_kind),  // eclipse 1 / self_cure 3(自蕴累积阴诡色)
                                           // penetrate 2 / shroud 1 / reverse 5
    source: XpGainSource::Action {
        plan: "dugu_v2",
        action: skill_kind.as_str(),
    }
}
```

PracticeLog 累积驱动 QiColor **阴诡色**（worldview §六:618）演化，由 plan-multi-style-v1 ✅ 已通的机制接管。但毒蛊**自蕴**产生的是**永久不可洗的染色**——区别于其他流派的可洗染色，本 plan P1 自行扩展 `cultivation::QiColor`（multi-style-v1 finished 模块）补 `permanent_lock_mask: HashSet<ColorKind>` 字段（dugu-v2 自蕴累积时写入，洗染色函数检查 mask 跳过该色）。注：plan-color-v1 不存在（被 plan-multi-style-v1 取代），永久 lock 字段归 dugu-v2 自身实装。

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 动画 | `bong:dugu_needle_throw` | 新建 JSON | P2 | 飞针掷出，priority 1000（战斗层）|
| 动画 | `bong:dugu_self_cure_pose` | 新建 JSON | P2 | 服毒姿态（端碗 + 仰脖 + 静坐），priority 300（姿态层）|
| 动画 | `bong:dugu_shroud_activate` | 新建 JSON | P2 | 双手交叉胸前，全身阴影渐现，priority 400（进阶姿态）|
| 动画 | `bong:dugu_pointing_curse` | 新建 JSON | P2 | 化虚级远指，单指点向远方，priority 1500（高阶战斗）|
| 粒子 | `DUGU_DARK_GREEN_MIST` ParticleType + Player | 新建 | P2 | 阴诡色暗绿雾，自蕴气息周围 + 化虚倒蚀余响 |
| 粒子 | `DUGU_TAINT_PULSE` ParticleType + Player | 新建 | P2 | 受害者身上脉动可视（仅高境 inspect 可见时渲染）|
| 粒子 | `DUGU_REVERSE_BURST` ParticleType + Player | 新建 | P2 | 倒蚀引爆瞬间在受害者位置爆开 |
| 音效 | `dugu_needle_hiss` | recipe 新建 | P3 | layers: `[{ sound: "entity.spider.hurt", pitch: 1.5, volume: 0.4 }, { sound: "block.fire.extinguish", pitch: 1.2, volume: 0.3, delay_ticks: 1 }]`（针刺低频嘶 + 蛇音）|
| 音效 | `dugu_self_cure_drink` | recipe 新建 | P3 | layers: `[{ sound: "entity.witch.drink", pitch: 0.8, volume: 0.5 }]`（服毒声）|
| 音效 | `dugu_curse_cackle` | recipe 新建 | P3 | layers: `[{ sound: "entity.witch.celebrate", pitch: 0.7, volume: 0.6 }, { sound: "ambient.cave", pitch: 1.2, volume: 0.3, delay_ticks: 5 }]`（化虚倒蚀远程嘲笑/嗤笑）|
| HUD | `DuguTaintWarningHud` | 新建 | P2 | 自身被蚀针中后角落红点 + qi_max 衰减速率提示 |
| HUD | `DuguTaintIndicator` | 新建 | P2 | 自身阴诡色 % + 形貌异化分级（ASCII 进度条）|
| HUD | `RevealRiskHud` | 新建 | P2 | 当前招式暴露概率显示（招式悬停时）|
| HUD | `SelfCureProgressHud` | 新建 | P2 | 自蕴累积（已服食次数 / 阴诡色当前 / 下一目标 %）|

---

## §4.5 P1 测试矩阵（饱和化测试）

下限 **120 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_eclipse` | 7 受害者境界 × 即时扣 / 永久阈值跨档（醒灵-凝脉无持续 / 固元短期 / 通灵+ 永久）+ 自蕴加成各档 + qi 不足 reject + cooldown | 25 |
| `cast_self_cure` | 阴诡色累积曲线 7 档 + 服食打断回滚 + 形貌异化阈值触发（≥60% dugu_self_revealed）+ 永久不可洗确认 + plan-botany-v2 毒草消耗 | 18 |
| `cast_penetrate` | 已植入 TaintMark 触发联级 + 无标记 reject + 7 境界倍率 + 化虚 zone 量级 + 多目标扫描 | 18 |
| `cast_shroud` | 7 境界遮蔽强度 + 伪造 qi_color override IdentityProfile + 持续 ticks + 主动关闭 + 化虚永久维持 | 15 |
| `cast_reverse` | 化虚专属判定 + 通灵以下「指无应」+ 引爆 zone 扫描所有 TaintMark + 阴诡色累加 + 绝壁劫触发链 | 18 |
| `reveal_probability` | 暴露公式 7 境界 × 距离 3 档 × 受害者境界 ×3 + DuguRevealedEvent emit + IdentityProfile 写入 | 14 |
| `permanent_decay_persistence` | 通灵+ 永久标记跨 server restart 持久化 + 受害者死亡清除 + 解蛊药中和 | 12 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/combat/dugu_v2/` ≥ 120。守恒断言：脏真元注入后 99% 散回受害者所在 zone qi 静态值（worldview §二 末法分解），但永久标记是**结构改变非守恒流量**——不计入守恒账本。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 化虚倒蚀触发绝壁劫

- **A**：保留（同涡心化虚一致格调，化虚毒蛊师每次清算 = 引天道下来）
- **B**：去掉（毒蛊师本就够阴，再加绝壁劫过苛）

**默认推荐 A** —— worldview §三:78 物理一致 + 跟涡心化虚同框格。化虚级是孤注，应有 trade-off。

### #2 暴露概率公式

- **A**：base 醒灵 5% / 化虚 0.2%（默认推荐，与 §0 一致）
- **B**：base 醒灵 8% / 化虚 0.05%（更阴险，化虚几乎不暴露 + 一旦暴露社会代价极重）
- **C**：base 醒灵 3% / 化虚 1%（更温和）

**默认推荐 A**。B 虽更阴但化虚毒蛊师全服几乎无对策，可能破坏 PVP 平衡。

### #3 自蕴累积曲线（边际递减，0→90% 总累计 ~100h）合理吗

当前公式：`daily_gain = 1.5%/h × (1 - current%/90)² × hours_today`，每日上限 6h。境界不影响速度（worldview §六:625 染色物理基础对所有境界一致）。

- **A**：保留（新手 5h 出 5% 主色手感快 + 高境瓶颈拉到 100h 边际递减）
- **B**：陡度更平缓（base 1.0%/h，总累计 ~150h）
- **C**：陡度更陡（base 2.0%/h，总累计 ~70h，加快爽度）

**默认推荐 A**。每日上限 6h 服食可能造成"挂机刷"，留 P0 telemetry 校准。

### #4 流派识别痕迹三种统一为 `DuguTaintField` 还是各自独立

- **A**：统一接口（同 worldview §二 EnvField 接口）
- **B**：各自独立 component（受害者 TaintMark / caster SelfCureAura / zone ReverseAftermathCloud）

**默认推荐 B** —— 三种痕迹的物理寄主不同（受害者侧 / 施法者侧 / 环境侧），强行统一会增加耦合度。但接口签名应该一致（都走 EnvField trait）。

### #5 自蕴专属毒草是新建 dugu 专属物种还是复用 plan-botany-v2 17 物种

- **A**：复用（已有的"赤髓草"、"血淋藤"等毒草特性已够用）
- **B**：新建 3-5 种 dugu 专属毒草（如"夜哭蛇腹叶"、"蚀骨菌"等）→ 派生 plan-botany-v3 子任务

**默认推荐 A**，待 P0 与 plan-botany-v2 维护者确认现有毒草特性是否够用，若不够再 B。

### #6 永久 lock 字段实装位置

毒蛊自蕴产生的阴诡色不可洗，需扩展 `cultivation::QiColor`（plan-multi-style-v1 ✅ finished 模块）补永久 lock 字段。**plan-color-v1 不存在**（被 plan-multi-style-v1 取代），所以无第三方 plan 接管。

- **A**：本 plan dugu-v2 P1 内自行扩展 QiColor 加 `permanent_lock_mask: HashSet<ColorKind>`（推荐，归属清晰）
- **B**：单独发 PR 改 plan-multi-style-v1 模块（如果 vN+1 立项）
- **C**：未来其他流派若需永久 lock 字段时，提取为通用模块或派生新 plan（额外开销大）

**默认推 A** —— dugu-v2 是当前唯一产生永久不可洗染色的流派，归属逻辑上属本 plan。其他流派若有类似需求 vN+1 时再讨论是否提取通用模块。

### #7 TaintMark 持久化到玩家死亡或永久持久化

- **A**：受害者死亡清除（重生后清白）
- **B**：永久持久化（甚至跨多周目，写入亡者博物馆作为生前印记）
- **C**：受害者死亡清除 + 写入 LifeRecord（亡者博物馆可读但新角色不继承）

**默认推荐 C** —— 既符合 worldview §十二 多周目机制，又保留毒蛊作为"生前隐患"的叙事价值。

---

## §6 进度日志

- **2026-05-05** 骨架立项，承接 plan-dugu-v1 ✅ finished（PR #126 commit 44a6ff9b 归档；shoot_needle P0 已实装）。
  - 设计轴心：脏真元 ρ=0.05 + 永久阈值分三档（低3段 HP/qi / 固元短期 / 通灵+ 永久）+ 自蕴（自身经脉养成毒源，非养虫）+ 暴露概率系统 + 阴诡色形貌异化（永久不可洗）
  - 五招完整规格 7 档威力表锁定（蚀针 / 自蕴 / 侵染 / 神识遮蔽 / 倒蚀）
  - **严守 worldview**：去除"蛊母 / 蛊虫 / 虫卵"等偏离虫子叙事，"蛊"仅作汉字"诡毒"意。worldview §五 + §六 正典完整对齐
  - 化虚质变：远程倒蚀引爆 zone 内所有永久标记 + 触发绝壁劫
  - 流派识别痕迹（毒蛊专属 EnvField）：脏真元残留（受害者侧）+ 自蕴气息（毒蛊师周身）+ 倒蚀余响（受害者位置）
  - worldview 锚点对齐：§三:78 + §三:368 + §四:506 + §五:421-427/520-535 + §六:618-621/625 + §十一:947-970/967 + §K
  - qi_physics 锚点：等 patch P0/P3 完成后接入；脏真元注入走 qi_physics::collision，倒蚀 zone 量级走 patch P3 加新算子 reverse_burst_all_marks
  - SkillRegistry / PracticeLog / HUD / 音效 / 动画 全部底盘复用，无新建框架
  - 待补：与 plan-style-balance-v1 ρ/W 矩阵对齐 / 扩展 cultivation::QiColor 加 permanent_lock_mask 字段（plan-multi-style-v1 ✅ finished 模块，本 plan P1 自行实装）/ plan-tribulation-v1 化虚倒蚀绝壁劫触发链 / plan-identity-v1 暴露后社会反应 / plan-botany-v2 自蕴毒草确认 / plan-craft-v1 蚀针 + 自蕴煎汤配方注册
- **2026-05-06** 审核修订（用户提出三点）：
  - **plan-color-v1 不存在**（被 plan-multi-style-v1 取代）→ 删除 6 处引用，永久 lock 字段归 dugu-v2 P1 自行扩展 cultivation::QiColor
  - **自蕴时间方向错 + 总量太大**：原表"醒灵 100h / 化虚 15h"违反"新手快高手慢"直觉。改为边际递减曲线 `daily_gain = 1.5%/h × (1 - current%/90)²`，0→90% 总累计 ~100h（与用户意图一致）
  - **自蕴跟境界绑定也错**：worldview §六:625 染色物理对所有境界一致 + §五:537 流派由组合涌现无门禁——醒灵也能慢慢成毒源，只是招式威力受永久阈值分级限制
  - 同步加 plan-craft-v1 反向被依赖（蚀针 + 自蕴煎汤通过通用手搓 tab 注册配方）

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：5 招对应 server/agent/client 模块路径
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test combat::dugu_v2` + 测试数 / `narration-eval` 5 招通过率 / WSLg 联调实录 / 暴露后社会反应实测
- **跨仓库核验**：server 5 招 SkillRegistry 注册 / agent 5 招 narration runtime + 暴露江湖传闻 / client 4 HUD + 3 粒子 + 4 动画 + 3 音效 / IdentityProfile DuguRevealedEvent consumer
- **遗留 / 后续**：cultivation::QiColor permanent_lock_mask 字段扩展（dugu-v2 P1 自身）/ telemetry 校准（plan-style-balance-v1）/ 自蕴毒草扩展（plan-botany-v3）/ 多周目 TaintMark 跨角色规则（plan-multi-life-v1）/ 蚀针 + 煎汤配方注册（plan-craft-v1）
