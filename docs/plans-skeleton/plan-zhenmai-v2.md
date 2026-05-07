# Bong · plan-zhenmai-v2 · 骨架

截脉·震爆功法**五招完整包**：动画 / 特效 / 音效 / 伤害 / 真元消耗 / 反噬 / 客户端 UI 全流程。承接 `plan-zhenmai-v1` ✅ finished（PR #122 commit 1ae7fd88 归档；P0 极限弹反已实装于 plan-combat-no_ui）—— v2 引入**音论物理**（worldview §P 定律 5 + cultivation-0002 §音论）+ **血肉反应装甲**（worldview §五:432-436 用血条保真元）+ **化虚专属绝脉断链**（worldview §四:319 主动 SEVERED 一条经脉，60s 内对选定攻击类型**反震效率 ×3**，K_drain 破例 0.5 → 1.5；**不是免疫**，受击仍命中但反震高效）+ **5 招完整规格**（极限弹反 / 局部中和 / 多点反震 / 护脉 / 绝脉断链），无境界 gate 只有威力门坎。

**世界观锚点**：`worldview.md §五:432-436 截脉/震爆流核心定义`（血肉反应装甲 + 弹反窗口 + 接触式中和）· `§五:464 primary axis 弹反窗口 + 污染真元中和效率`· `§五:570 暴烈色`（震爆爆破倾向）· `§六:619 暴烈色染色谱`（真元带电间歇放电 + 击穿护体真气）· `§四:283-300 三层级联模型 + 流量公式 + 异体排斥 / 排异 tick`· `§四:319 SEVERED 永久残废除非外力接续`（绝脉断链物理依据）· `§三:78 化虚天道针对`（化虚级 SEVERED 触发天道注视）· `§K narration 沉默`

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §音论`（高频共振反馈 + 接触面 C 决定反震集中度）· `peoples-0006 战斗流派源流` 体修源流（截脉适合体修近战狂人）

**前置依赖**：

- `plan-qi-physics-v1` P1 ship → 音论 R_jiemai / β=0.6 走 `qi_physics::collision::reverse_clamp`
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 ρ/W/β 矩阵实装（截脉 β=0.6 / W vs 4 攻 [0.5, 0.7, 0.2, 0.0]）
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ + `plan-hotbar-modify-v2` 🆕（P1 SkillConfig 通用底盘 + floating window，⑤ 绝脉断链是其首个使用方）+ `plan-multi-style-v1` ✅
- `plan-combat-no_ui` ✅ → 极限弹反 P0 已实装在 combat 主模块（v2 迁出至 zhenmai_v2）
- `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅
- `plan-cultivation-canonical-align-v1` ✅ → Realm + 经脉拓扑选择（绝脉断链需精确指定经脉）

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 招的 W/β 数值进矩阵（截脉 β=0.6 / 专克器修 W=0.7 / 失效 vs 毒蛊 W=0.0）
- `plan-tribulation-v1` ⏳ → 化虚级绝脉断链触发天道注视累积
- `plan-narrative-political-v1` ✅ active → 化虚截脉师主动 SEVERED 经脉的江湖传闻（断脉求生）
- `plan-multi-life-v1` ⏳ → 永久 SEVERED 跨周目处理（重生新角色不继承旧 SEVERED）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, contamination, qi_color }` / `cultivation::MeridianSystem` / `qi_physics::ledger::QiTransfer` / `qi_physics::collision::reverse_clamp`（K_drain ≤ 0.5） / `combat::Wounds`（HP 自伤）/ `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm`
- **出料**：5 招 `ZhenmaiSkillId` enum 注册到 SkillRegistry / `JiemaiParryEvent`（扩展 v1 实装含 7 档威力）/ `LocalNeutralizeEvent` 🆕 / `MultiPointBackfireEvent` 🆕 / `MeridianHardenEvent` 🆕 / `MeridianSeveredVoluntaryEvent` 🆕（绝脉断链）/ `BackfireAmplificationActiveEvent` 🆕（60s 反震效率 ×3 激活）/ `JiemaiBackfireBloodSpray` 🆕（皮下震爆痕迹）
- **共享类型**：`StyleDefense` trait（qi_physics::traits）/ `MeridianSeveredVoluntary` component（永久标记 + 持有此 component 的玩家不能再 cast 与该经脉关联的招式，迁出至 plan-meridian-severed-v1 通用 component）/ `BackfireAmplification` component（60s 反震 K_drain 破例 0.5→1.5 的攻击类型 + 剩余 ticks）
- **跨仓库契约**：
  - server: `combat::zhenmai_v2::*` 主实装（v1 极限弹反迁入 + v2 新 4 招）/ `schema::zhenmai_v2`
  - agent: `tiandao::zhenmai_v2_runtime`（5 招 narration + 化虚断脉求生叙事 + 弹反成功"血肉之躯"叙事 + 多点反震群战叙事）
  - client: 5 动画 + 3 粒子 + 4 音效 recipe + 4 HUD 组件
- **worldview 锚点**：见头部
- **qi_physics 锚点**：弹反 K_drain ≤ 0.5 走 `qi_physics::collision::reverse_clamp`；多点反震 C_contact 分散走 `qi_physics::field::multi_point_dispersion` 🆕（patch P3 加新算子）；护脉硬化走 `qi_physics::collision::flow_modifier`；绝脉断链 SEVERED 走 `qi_physics::field::sever_meridian` 🆕（patch P3 加）；**禁止 plan 内自己写音论 / 反震 / SEVERED 公式**

---

## §0 设计轴心

- [ ] **血肉派定调（worldview §五:432-436 + §五:464）**：截脉用**血肉 / 经脉换 qi 免伤**，区别于物资派（替尸钱包代价）和物理改造派（涡流环境改造）。每招都有真实物理代价：
  - ① 弹反成功 → HP 自伤 3-8（皮下震爆物理代价）
  - ② 局部中和 → 自身真元 10:15 比例亏损（worldview §四:298）
  - ③ 多点反震 → 全身 contam 累积分散
  - ④ 护脉 → 经脉硬化反弹风险
  - ⑤ 绝脉断链 → **永久 SEVERED 一条经脉**（化虚专属极限代价）

- [ ] **音论物理（worldview §P 定律 5 + cultivation-0002 §音论）**：
  ```
  R_jiemai = 反震系数（固定）
  C_contact = 接触面积（决定反震集中度）

  反震伤害 = R_jiemai × E_in × C_contact

  关键性质：
    单点接触（器修单根骨刺刺入）：C_contact 高 → 反震集中 → 攻方载体直接被打碎
    多点接触（体修贴脸全身近）：C_contact 低但分布广 → 攻方多处神经过载但本体经脉还在

  W 表（worldview §P.3）：
    截脉 vs 体修：W=0.5（贴脸接触面广分散）
    截脉 vs 器修：W=0.7（单点反震集中，**专克**）
    截脉 vs 地师：W=0.2（陷阱无瞬时接触）
    截脉 vs 毒蛊：W=0.0（脏真元持续低强度激发不了音论高频共振阈值，**完全失效**）
  ```

- [ ] **化虚级绝脉断链（worldview §四:319 + §五:432 + §P 定律 5 音论）**：截脉的"质变"在化虚级专属——主动 SEVERED 一条经脉换 **60s 反震效率 ×3**。**不是免疫**，是断脉空隙引导能量到反震路径的高效率反震。物理推导：
  ```
  正常 K_drain ≤ 0.5 (worldview §P clamp)
  绝脉断链激活 → 选定攻击类型的 K_drain 破例 0.5 → 1.5 (×3 倍)
  物理依据: 断脉空隙创造 §P 音论高频共振路径
  受击仍正常命中（伤害走 §P 矩阵），但反震攻方真元 ×3
  自身受伤减半（断脉引能转化部分能量为反震）
  ```
  - 普通流派化虚级（涡流涡心紊流场 / 毒蛊倒蚀引爆）= 主动战斗手段
  - **截脉化虚级 = 主动残废自己**（worldview §四:319 SEVERED 永久残废）→ 换 60s 反震高效
  - 哲学：截脉师在生死关头**用经脉换反震效率**，跟血肉派定调一致——身体损伤换战场存活

- [ ] **专属物理边界 = 瞬时痕迹（不是长期场）**：跟其他流派最大区别：
  - 涡流：紊流场 5min 散尽（长期场）
  - 毒蛊：脏真元残留 30min 衰减（长期场）
  - 替尸：蜕落物 30min 腐烂（长期场）
  - **截脉：皮下血雾 + 中和余响 5-10s 散尽**（瞬时即逝）—— 血肉只在被刺破的瞬间反应，过后就愈合，物理一致性
  
  痕迹：
  - **皮下震爆痕迹**（worldview §五:434）：受击点皮下血腥爆裂，敌方近战可见血雾喷出（5-10s 散尽）
  - **接触中和余响**（弹反成功瞬间）：攻方位置短暂可见 contam 飞散粒子（2-3s）
  - **绝脉断链光痕**（化虚级一次性）：选定经脉位置短暂金光（1-2s 散尽，但 SEVERED 永久）

- [ ] **反噬阶梯（血肉派 + 含 SEVERED）**：跟涡流类似但物理不同：
  | 触发条件 | 反噬级 |
  |---|---|
  | 弹反成功（每次） | HP 自伤 3-8 + contam +1% / 累积 |
  | 多点反震 < 50% 维持 | contam +0.01/s + HP -1/s |
  | 多点反震 50-100% 维持 | MICRO_TEAR（受冲击点经脉流量 ×0.85 / 5min） |
  | 护脉超 80% 维持 | TORN（该经脉 ×0.5 / 30min） |
  | 弹反失败 + 受击 ≥ 3 次 / 30s | TORN（多处经脉同时损伤） |
  | **化虚绝脉断链 cast** | **永久 SEVERED 选定经脉**（不可逆，需上古接经术残卷） |
  | 极端：化虚弹反时机错配 + 多点反震双开 | SEVERED（极小概率，需玩家主动作死） |

- [ ] **无境界 gate，只有威力门坎**（worldview §五:537）：5 招都允许任何境界 cast，三层物理自然惩罚：
  - qi_current 不足 → server `Rejected{ QiInsufficient }`
  - **境界决定威力**（K_drain 反吸比例 / 反震点数 / 硬化抗性 / 中和兑换率），低境威力近零
  - **绝脉断链化虚专属** → 通灵以下 cast 仍 SEVERED 一条经脉但**没有反震 ×3 加成**（HUD「断脉无应」）—— 自然惩罚，物理代价已付但收益没有

- [ ] **熟练度生长 — 二维划分（通用机制）**：
  ```
  境界 = 威力上限（K_drain / 反震点数 / 硬化抗性 / 中和兑换率）
  熟练度 = 响应速率（冷却 / 弹反窗口 / cast time）

  公式（线性递减/递增）：
    cooldown(skill_lv) = cooldown_base + (cooldown_min - cooldown_base) × clamp(lv/100, 0, 1)
    window(skill_lv)   = window_base   + (window_max  - window_base)  × clamp(lv/100, 0, 1)

  熟练度来源：plan-skill-v1 SkillSet.skill_lv（每次 cast → SkillXpGain event）
  100h 路径目标：弹反成功 ~1000-5000 次 ≈ skill_lv 50-100，需调整 xp_to_next 曲线
  ```
  
  哲学：醒灵但弹反 lv 50 = 弹反窗口 175ms / 冷却 17s（练得多但池小）；化虚但弹反 lv 0 = 窗口 100ms / 冷却 30s（化虚老怪从未练过截脉，反应像新手）。worldview §五:537 流派由组合涌现 + §五:506 末土后招原则的物理化身——**能力跟练得多有关，不绑死境界**

  注：本机制是 v2 流派 plan 通用设计——本 plan 是首个引入，其他流派 v2（woliu / dugu / tuike）应在升 active 前回填或留 vN+1 统一

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：5 招数值表锁定 + 弹反窗口 200ms 各境界 K_drain 分级 + 化虚绝脉断链选定经脉机制（手三阴/三阳/足三阴/三阳/任督）design + §5 五决策门收口 + qi_physics 接入面定稿 + 与 plan-style-balance-v1 W/β 矩阵对齐（截脉 β=0.6 / W vs 4 攻 [0.5, 0.7, 0.2, 0.0]） | 数值矩阵 + 物理公式落 plan §2 / qi_physics 算子接入面定稿 |
| **P1** ⬜ | server `combat::zhenmai_v2::*` 5 招 logic + v1 极限弹反迁入 + 音论物理（R_jiemai / C_contact）+ 多点反震分散 + 护脉硬化 + 绝脉断链永久 SEVERED 写入 MeridianSystem + 60s 反震 K_drain 破例 0.5→1.5 判定（按攻击类型分类）+ 散功期间敌方攻击仍正常命中测试 + qi_physics 算子调用 + ≥100 单测 | `cargo test combat::zhenmai_v2` 全过 / 守恒断言（弹反 K_drain 走 ledger）/ SEVERED 跨 server restart 持久化测试 / `grep -rcE '#\[test\]' server/src/combat/zhenmai_v2/` ≥ 100 |
| **P2** ⬜ | client 5 动画（弹反姿态 / 多点反震双臂展开 / 护脉手掌覆盖 / 局部中和指点经脉 / 绝脉断链一手按断另一经脉位）+ 3 粒子（JIEMAI_BURST_BLOOD / JIEMAI_NEUTRALIZE_DUST / JIEMAI_SEVER_FLASH 化虚级金光）+ 4 HUD 组件 | render_animation.py 验证 / WSLg 实跑 5 招视觉确认 / 弹反 200ms 窗口指示 HUD 实测时机感 |
| **P3** ⬜ | 4 音效 recipe（parry_thud / neutralize_hiss / shield_hum / sever_crack 化虚断脉清脆裂声）+ agent 5 招 narration template + 化虚断脉求生叙事 + 弹反成功"血肉之躯"叙事 + 多点反震群战叙事 | narration-eval ✅ 5 招 + 化虚断脉极限叙事 全过古意检测 |
| **P4** ⬜ | PVP telemetry 校准 / 暴烈色 hook（PracticeLog → QiColor 暴烈色累积演化）/ 跟 baomai-v2 越级+全力一击 vs 截脉弹反实战测试（化虚体修 vs 化虚截脉互克验证）/ 跟 dugu-v2 化虚倒蚀的反制路径（化虚截脉 ⑤ 绝脉断链选"脏真元类"反震 ×3 vs dugu 倒蚀） | 7 流派 4×3 攻防对位截脉通过 / 化虚级 hard counter dugu 实战测试 / 暴烈色长期累积演化测试 |

**P0 决策门**：完成前 §5 五决策必须有答案。

---

## §2 五招完整规格

### ① 极限弹反（已 v1 ✅ P0，v2 校准 7 档威力 + 熟练度生长）

worldview §五:434 + §P 定律 5。接触瞬间触发，必须接触（区别 woliu 瞬涡负压式）。

**境界维度（威力上限 = K_drain / 自伤）**：

| 境界 | K_drain（反吸） | 自伤 HP | 适用攻击 |
|---|---|---|---|
| 醒灵 | 0.05 | 8 | 体修近战可弹 |
| 引气 | 0.15 | 8 | + 毒蛊蚀针（W=0.0 仅弹但反吸 0） |
| 凝脉 | 0.30 | 6 | + 暗器载体 |
| 固元 | 0.40 | 5 | + 阵法触发 |
| 通灵 | 0.50（clamp） | 4 | 全面 |
| 半步化虚 | 0.50 | 4 | 全面 |
| 化虚 | 0.50 | 3 | 全面 |

**熟练度维度（响应速率 = 冷却 / 弹反窗口）**：

| skill_lv | 弹反窗口 | 冷却 |
|---|---|---|
| 0（生手） | 100ms | 30s |
| 25 | 138ms | 23.75s |
| 50 | 175ms | 17.5s |
| 75 | 213ms | 11.25s |
| 100（精通） | 250ms | 5s |

公式（线性插值）：
```
window_ms = 100 + 150 × clamp(skill_lv/100, 0, 1)
cooldown_s = 30 - 25 × clamp(skill_lv/100, 0, 1)
```

**caster qi 消耗**：8（一次性，不随熟练度变化）
**vs 涡流瞬涡对比**：截脉肉接（自伤换免伤），瞬涡负压吃（不自伤但 5s 冷却 + 200ms 时机准）。两者覆盖不同距离（截脉接触 / 瞬涡 ≤2 格场延伸），互补不重叠
**worldview 锚**：§五:432-436 + §P 定律 5
**实战意象**：化虚老怪首次试截脉 → 100ms 窗口 + 30s 冷却（弹不到 + 等不起）；醒灵苦修截脉 5 年 lv 100 → 250ms 窗口 + 5s 冷却（轻松弹反凡铁刀，K_drain 0.05 但准）

### ② 局部中和（事后清污）

worldview §四:298 排异 tick + 10:15 比例亏损物理。主动 cast，选择 1 条经脉清掉异种真元 contam。

| 境界 | qi/% contam 兑换 | 单次推动上限 |
|---|---|---|
| 醒灵 | 18 qi / 1% | 1% |
| 引气 | 16 qi / 1% | 2% |
| 凝脉 | 14 qi / 1% | 4% |
| 固元 | 12 qi / 1% | 7% |
| 通灵 | 10 qi / 1% | 10% |
| 半步化虚 | 9 qi / 1% | 12% |
| 化虚 | 8 qi / 1% | 15% |

**机制**：主动选择 1 条经脉（inventory inspect UI 内点击）+ qi 消耗 → 该经脉 contam 减少。**专属：高境截脉可中和 dugu 短期 qi_max 衰减**（固元注入的 24h 标记），但**不能中和永久标记**（那需替尸 ③ 上古伪皮 hard counter 或本 plan ⑤ 绝脉断链选"脏真元类"反震 ×3 反吸 dugu 真元）
**冷却（按熟练度）**：lv 0 → 10s / lv 50 → 6.5s / lv 100 → 3s（线性）
**反噬**：超伪皮承载？无伪皮场景，但失败（contam 反流）→ 自身经脉额外 +5% / 5min
**vs 替尸 ③ 转移污染对比**：替尸推到伪皮带走（物资代价）；截脉直接中和（自身真元 10:15 比例亏损）

### ③ 多点反震（群战 1v 多）

worldview §P 定律 5 多点接触 C 低但分布广。全身 5-8 处皮下小爆触发护体真气覆盖。

| 境界 | 反震点数 | 持续 | qi 启动 + qi/s | 反震 K_drain |
|---|---|---|---|---|
| 醒灵 | 3 点 | 3s | 12 + 1 | 0.05 |
| 引气 | 4 点 | 4s | 12 + 1 | 0.10 |
| 凝脉 | 5 点 | 5s | 15 + 1.5 | 0.20 |
| 固元 | 6 点 | 7s | 18 + 2 | 0.30 |
| 通灵 | 7 点 | 9s | 22 + 2.5 | 0.35 |
| 半步化虚 | 8 点 | 10s | 25 + 3 | 0.35 |
| 化虚 | 8 点 | 12s | 28 + 3.5 | 0.35 |

**机制**：开启期间任何接触触发自动反震（K_drain 比 ① 单点 0.5 低，因为分散，按 worldview §P 定律 5 C_contact 分散）+ 自伤分散到 5-8 处（每处 -1 HP / 触发）。期间不能再 cast ①（互斥）—— worldview §P 物理：多点 C 低 vs 单点 C 高，不能同时
**冷却（结束后，按熟练度）**：lv 0 → 30s / lv 50 → 19s / lv 100 → 8s（线性）
**适合**：1v 多围攻 / 群战
**worldview 锚**：§P 定律 5 "多处神经过载但本体经脉还在"

### ④ 护脉（事前防）

worldview §四:286 流量公式 `effectiveFlow = currentFlow × damage.flowMultiplier × (1 - contamination)` 物理化身——硬化期间 damage.flowMultiplier 提升（损伤抗性）。

| 境界 | 单条经脉损伤抗性 | 持续 | qi 启动 + qi/s |
|---|---|---|---|
| 醒灵 | ×0.85 | 10s | 8 + 0.5 |
| 引气 | ×0.80 | 15s | 8 + 0.5 |
| 凝脉 | ×0.65 | 20s | 10 + 0.7 |
| 固元 | ×0.50 | 30s | 12 + 1.0 |
| 通灵 | ×0.35 | 45s | 15 + 1.5 |
| 半步化虚 | ×0.25 | 60s | 18 + 2.0 |
| 化虚 | ×0.20 | 90s + 可叠 2 条 | 22 + 2.5（每条） |

**机制**：主动 cast → 选择 1 条经脉（化虚可叠 2 条）→ 期间该经脉收到的 damage.flowMultiplier 损伤减少。可与 ① 弹反 / ② 中和并存（不互斥）
**反噬**：维持 ≥ 80% 上限 → 经脉硬化反弹，损伤反流 +5%。化虚可叠 2 条护脉但反弹概率 ×1.5
**冷却（结束后，按熟练度）**：lv 0 → 15s / lv 50 → 10s / lv 100 → 5s（线性）

### ⑤ 绝脉断链 — 化虚专属（主动 SEVERED 换 60s 反震效率 ×3）

worldview §四:319 SEVERED 永久残废 + §五:432 血肉反应装甲 + §P 定律 5 音论物理化身。**化虚专属**——通灵以下 cast 仍 SEVERED 一条经脉但没有反震 ×3 加成（HUD「断脉无应」）。

**重要：不是 MMO 式"免疫"**——受击仍正常命中（伤害走 §P 物理矩阵），但反震攻方真元 ×3 + 自身受伤减半。worldview 没有"免疫"概念，只有不同效率的反震/中和/距离衰减。

| 境界 | 选定经脉 | 60s 内反震加成 | 自身受伤减少 | qi 启动 |
|---|---|---|---|---|
| 醒灵-通灵 | 任选 1 条 | **无加成**，仅 SEVERED | — | — / **HUD「断脉无应」** |
| 半步化虚 | 任选 1 条 | K_drain 0.5 → 1.2（×2.4）| -30% | 60 |
| 化虚 | 任选 1 条 | K_drain 0.5 → 1.5（×3，**破例**）| -50% | 50 |

**4 类反震加成攻击类型**（InspectScreen「功法」tab 该招详情卡 → 齿轮 → floating window 预配置 `SkillConfig.backfire_kind` 选 1，详见下方"操作"段；底盘归 plan-hotbar-modify-v2 🆕 P1 交付）：

1. **真元类**：体修注入 / 涡流抽取 / 截脉反震（vs zhenmai 同源攻击反震 ×3）
2. **物理载体类**：暗器骨刺 / 凡器砍劈（载体本身反震碎裂）
3. **脏真元类**：dugu 蚀针 / 阴诡色侵染（**反制化虚倒蚀的关键路径**——反吸 dugu 真元 + 自身少受 50%，但永久标记仍生效）
4. **阵法类**：地师诡雷 / 缚阵触发

**操作**（接 plan-hotbar-modify-v1 ✅ + `combat::Casting` 标准路径）：

- **InspectScreen 预配置（接 plan-hotbar-modify-v2 🆕 §1.4 SkillConfig Floating Window）**：玩家在 InspectScreen (I 键)「功法」tab → 该招行 → 详情卡右下齿轮 → 弹出 floating window 浮窗，dropdown 选 `(meridian_id, backfire_kind)`——20 条经脉 × 4 类攻击 = 80 种变种 → 保存按钮发 `bong:skill/config_intent` 写入 `SkillConfigStore[skill_id]`
- **战时 cast**：拖到 1-9 槽 → 战斗时按数字键 → server 读 `SkillConfigStore` 快照到 `Casting.skill_config`（duration 走 cast time 一次倒计时；cast 中改 store 不影响当前 cast——快照保险；移动 / 受击中断走标准路径，中断 → SEVERED 不发生，业力代价不扣）
- **未配置保护**：`SkillConfig` 缺省或字段不全 → cast 失败 + HUD 红字"未选定经脉 / 攻击类型"，避免"按下去就 SEVERED 一条没选好的经脉"
- **cast 期间 floating window 强制收起**：client 监听 `Casting` 状态变化 → 若浮窗打开则强制 close + 齿轮按钮灰显（plan-hotbar-modify-v2 §1.4 双保险）
- **战中改选**：开 InspectScreen 重选期间 MC 原生 GUI 不暂停世界 / 玩家停手不能反应 / 敌人继续攻击——**自负风险**（MC 服务器 GUI 一致语义）
- **决策成本前置**：⑤ 是化虚一次性破釜沉舟招（一战通常只 cast 1 次），战前审慎选好；战时按下去就生效不能犹豫，符合"破釜沉舟"语义

**机制**：
- 主动 cast → 服务端读 `SkillConfig.meridian_id` + `SkillConfig.backfire_kind` 进入 Casting → cast 完成立即处置
- 选定经脉立即 **永久 SEVERED**（不可逆，除非上古"接经术"残卷 worldview §十六.三 一次性脆化或医者 NPC 接经术 plan-yidao-v1 🆕）
- 60s 内对选定攻击类型：
  - 受击命中 → 伤害走 §P 物理矩阵正常计算（**不是免疫**）
  - 反震攻方真元 ×3（K_drain 破例 0.5 → 1.5）
  - 自身受伤减少 50%（断脉引能转化部分能量为反震）
- 60s 后反震加成消失，但 SEVERED 永久

**反噬（永久身体损失）**：
- 手三阴/三阳 SEVERED → 该手不能持物 / 不能 melee 真元注入
- 足三阴/三阳 SEVERED → 移速 ×0.7 永久（worldview §四:255-256 腿部损伤 → 移速分级）
- 任督二脉 SEVERED → qi_max -10% 永久

化虚 qi_max 10700 ×10% = 1070 损失，仍能维持但池子永久缩水。**化虚截脉师每次断脉都是单方面战略损失**

**叙事意象**：化虚截脉师在生死关头突然按住自己一条经脉、咬破舌尖 → 该经脉应声而断 → 60s 内对手攻击如击虚空。但战后他将永远残废一手或一腿。worldview §三:78 化虚天道针对——主动 SEVERED 触发天道注视累积（连续 cast 3 次 → 触发"绝壁劫"，同涡心化虚 + 倒蚀化虚一致格调）

**反制化虚毒蛊师的关键路径**（与替尸 ③ 上古伪皮 / baomai ⑤ 散功 三选一并列）：
- 化虚毒蛊师倒蚀引爆永久 qi_max 衰减标记
- 化虚截脉师 ⑤ 绝脉断链选"脏真元类"反震 ×3 → 60s 内反吸 dugu 真元 + 自身受伤减半
- **dugu 倒蚀永久标记仍生效**（受全额命中），但反震高效率消耗 dugu 真元 + 自身受伤减半
- 代价：永久 SEVERED 一条经脉
- 跟其他反制路径对比（**全部物理可推导，无免疫机制**）：
  - tuike ③：烧物资（上古伪皮承伤 + 蜕落带走永久标记，物理是"标记转移"）
  - **zhenmai ⑤**：烧身体（永久 SEVERED 一条经脉，60s 反震 ×3 + 受伤 -50%，**仍受伤但反震高效**）
  - baomai ⑤：烧池子（永久 -50% qi_max，5s flow_rate ×10 拼命反杀，仍受伤）

---

## §3 数据契约

```
server/src/combat/zhenmai_v2/
├── mod.rs              — Plugin + register_skills + 迁入 v1 极限弹反
├── skills.rs           — ZhenmaiSkillId enum (Parry/Neutralize/MultiPoint/
│                                              HardenMeridian/SeverChain)
│                        + 5 resolve_fn
├── state.rs            — JiemaiParryState (200ms 窗口判定状态机) +
│                        MultiPointActive component (反震点数 + 持续 ticks +
│                                                    自伤累积) +
│                        MeridianHardenActive component (硬化经脉 + 持续 ticks) +
│                        MeridianSeveredVoluntary component (永久, 选定经脉 ID +
│                                                            severed_at_tick) +
│                        BackfireAmplification component (60s 反震 K_drain 破例 0.5→1.5
│                                                          的攻击类型 + 剩余 ticks，
│                                                          **不含免疫 flag**)
├── tick.rs             — parry_window_tick (200ms 极限判定) +
│                        multipoint_dispersion_tick (持续期间反震 +
│                                                    自伤分散) +
│                        harden_maintain_tick + selective_immunity_tick (60s 倒计时)
├── physics.rs          — 音论 R_jiemai × C_contact 计算 +
│                        多点反震分散 (调 qi_physics::field::multi_point_dispersion) +
│                        护脉 flow_modifier 调整 +
│                        绝脉断链 SEVERED 写入 MeridianSystem (调 qi_physics::field::sever_meridian) +
│                        反震 K_drain 破例放大判定（按攻击类型分类）
├── reveal.rs           — 暴烈色 PracticeLog 累积 (每招 amount 不同) +
│                        passive_buff hook
└── events.rs           — JiemaiParryEvent (扩展 v1) /
                          LocalNeutralizeEvent / MultiPointBackfireEvent /
                          MeridianHardenEvent / MeridianSeveredVoluntaryEvent /
                          SelectiveImmunityActiveEvent / JiemaiBackfireBloodSpray

server/src/schema/zhenmai_v2.rs  — IPC schema 5 招 + 永久 SEVERED + 反震加成 payload

agent/packages/schema/src/zhenmai_v2.ts  — TypeBox 双端
agent/packages/tiandao/src/zhenmai_v2_runtime.ts  — 5 招 narration +
                                                    化虚断脉求生叙事 +
                                                    弹反成功"血肉之躯"叙事 +
                                                    多点反震群战叙事 +
                                                    化虚断脉触发天道注视预兆

client/src/main/java/.../combat/zhenmai/v2/
├── ZhenmaiV2AnimationPlayer.java         — 5 动画播放
├── JiemaiBurstBloodParticle.java         — 皮下血雾粒子
├── JiemaiNeutralizeDustParticle.java     — 接触中和余响粒子
├── JiemaiSeverFlashParticle.java         — 绝脉断链化虚级金光
├── ParryWindowHud.java                   — 200ms 弹反窗口指示（攻击预兆触发短暂红框）
├── MeridianContamHud.java                — 每条经脉 contam 可视（局部中和决策）
├── ShieldedMeridianHud.java              — 护脉中的经脉高亮
└── SeveredMeridianListHud.java           — 永久 SEVERED 经脉列表（不可逆，警示性）

client/src/main/resources/assets/bong/
├── player_animation/zhenmai_parry.json
├── player_animation/zhenmai_neutralize.json
├── player_animation/zhenmai_multipoint.json
├── player_animation/zhenmai_harden.json
├── player_animation/zhenmai_sever_chain.json
└── audio_recipes/parry_thud.json + neutralize_hiss.json +
                  shield_hum.json + sever_crack.json
```

**SkillRegistry 注册**：

```rust
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("zhenmai.parry",         cast_parry);            // 极限弹反
    registry.register("zhenmai.neutralize",    cast_neutralize);       // 局部中和
    registry.register("zhenmai.multipoint",    cast_multipoint);       // 多点反震
    registry.register("zhenmai.harden",        cast_harden_meridian);  // 护脉
    registry.register("zhenmai.sever_chain",   cast_sever_chain);      // 绝脉断链
}
```

**PracticeLog 接入**：

```rust
emit SkillXpGain {
    char: caster,
    skill: SkillId::Zhenmai,
    amount: per_skill_amount(skill_kind),  // parry 1 / neutralize 1 / 
                                           // multipoint 2 / harden 1 / sever 5
    source: XpGainSource::Action {
        plan: "zhenmai_v2",
        action: skill_kind.as_str(),
    }
}
```

PracticeLog 累积驱动 QiColor **暴烈色**（worldview §六:619）演化，由 plan-multi-style-v1 ✅ 接管。暴烈色加成：worldview §六:619 "击穿护体真气 + 可远程小威慑" → 截脉弹反 K_drain 提升 5%（暴烈色 ≥ 30% 后）

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 动画 | `bong:zhenmai_parry` | 新建 JSON | P2 | 弹反姿态（手腕翻转抖刀 / 半步后撤），priority 1500（高阶战斗）|
| 动画 | `bong:zhenmai_neutralize` | 新建 JSON | P2 | 手指点经脉位（按胸/按腿），priority 800（中阶战斗） |
| 动画 | `bong:zhenmai_multipoint` | 新建 JSON | P2 | 双臂展开 + 全身红光波纹，priority 1200 |
| 动画 | `bong:zhenmai_harden` | 新建 JSON | P2 | 手掌覆盖单条经脉位置，priority 600（姿态层）|
| 动画 | `bong:zhenmai_sever_chain` | 新建 JSON | P2 | 一手按断另一经脉位 + 短暂咬牙姿态（化虚极限），priority 1800（高阶战斗）|
| 粒子 | `JIEMAI_BURST_BLOOD` ParticleType + Player | 新建 | P2 | 皮下血雾爆裂（弹反成功 / 多点反震） |
| 粒子 | `JIEMAI_NEUTRALIZE_DUST` ParticleType + Player | 新建 | P2 | 中和余响（contam 飞散 2-3s） |
| 粒子 | `JIEMAI_SEVER_FLASH` ParticleType + Player | 新建 | P2 | 绝脉断链化虚级金光（1-2s 散尽，但 SEVERED 永久）|
| 音效 | `parry_thud` | recipe 新建 | P3 | layers: `[{ sound: "block.anvil.land", pitch: 1.5, volume: 0.5 }, { sound: "entity.player.hurt", pitch: 1.2, volume: 0.3, delay_ticks: 1 }]`（弹反钝击 + 自伤）|
| 音效 | `neutralize_hiss` | recipe 新建 | P3 | layers: `[{ sound: "block.fire.extinguish", pitch: 1.0, volume: 0.4 }]`（中和嘶声）|
| 音效 | `shield_hum` | recipe 新建 | P3 | layers: `[{ sound: "block.beacon.activate", pitch: 1.2, volume: 0.4 }]`（护脉嗡鸣）|
| 音效 | `sever_crack` | recipe 新建 | P3 | layers: `[{ sound: "block.bone_block.break", pitch: 0.7, volume: 0.7 }, { sound: "entity.player.hurt", pitch: 0.6, volume: 0.5, delay_ticks: 2 }]`（化虚断脉清脆裂声 + 闷哼）|
| HUD | `ParryWindowHud` | 新建 | P2 | 攻击预兆触发短暂红框（200ms 弹反窗口指示）|
| HUD | `MeridianContamHud` | 新建 | P2 | 每条经脉 contam 可视（局部中和决策辅助）|
| HUD | `ShieldedMeridianHud` | 新建 | P2 | 护脉中的经脉高亮 + 持续时间倒计 |
| HUD | `SeveredMeridianListHud` | 新建 | P2 | 永久 SEVERED 经脉列表（绝脉断链历史，警示性）|

---

## §4.5 P1 测试矩阵（饱和化测试）

下限 **100 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_parry` | 200ms 窗口内/外 + K_drain 7 档 + 自伤 HP 7 档 + W vs 4 攻 (体修 0.5 / 器修 0.7 / 地师 0.2 / 毒蛊 0.0) + 5s 冷却 + 攻方真元清零 clamp | 22 |
| `cast_neutralize` | 7 境界 qi/% 兑换 + 推动上限 + 失败反流 +5% + 短期 qi_max 衰减中和（dugu 固元注入）+ 永久标记不能中和 | 18 |
| `cast_multipoint` | 7 境界点数 + 持续期间反震 + 自伤分散到 5-8 处 + 与 ① 互斥 + qi 不足时提前结束 | 18 |
| `cast_harden_meridian` | 7 境界硬化抗性 + 维持 ≥ 80% 反弹 + 化虚叠 2 条 + 多条同时维持 qi cost | 15 |
| `cast_sever_chain` | 化虚专属判定 + 通灵以下「断脉无应」+ 4 类反震加成攻击类型选定 + 永久 SEVERED 持久化（跨 server restart）+ 60s K_drain 破例 0.5→1.5 倒计 + 散功期间敌方攻击仍正常命中（伤害走 §P 矩阵）+ 反制 dugu 倒蚀实战测试（永久标记仍生效但反震高效消耗 dugu 真元）| 25 |
| `multipoint_dispersion` | C_contact 分散公式（worldview §P 定律 5）+ 多点 K_drain 比单点低 + 守恒断言 | 8 |
| `meridian_severed_persistence` | SEVERED 跨 server restart + 多周目重生不继承（plan-multi-life-v1 联调）+ 上古接经术残卷恢复 hook | 7 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/combat/zhenmai_v2/` ≥ 100。守恒断言：弹反 K_drain 走 `qi_physics::ledger::QiTransfer{from: attacker, to: defender, amount: K_drain × E_in}`。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 化虚级绝脉断链触发"绝壁劫"是否保留？

- **A**：保留（连续 cast 3 次 → 触发，跟涡心化虚 + 倒蚀化虚一致格调）
- **B**：去掉（截脉师本就血肉派代价极高，再加绝壁劫过苛）
- **C**：仅"任督二脉"断脉触发绝壁劫（任督是 worldview §三 主轴）

**默认推 A** —— worldview §三:78 物理一致 + 三个化虚专属招式（涡心 / 倒蚀 / 绝脉）同框格。化虚级是孤注，应有 trade-off

### #2 绝脉断链选定经脉的限制

- **A**：任选 20 条（手三阴 3 + 手三阳 3 + 足三阴 3 + 足三阳 3 + 任督 8）
- **B**：仅可选已通经脉（未通的不能断）
- **C**：仅可选正经（不能断奇经八脉）

**默认推 B** —— 物理上未通的经脉本就不导能量，断了无意义。但 UI 应高亮可选项

### #3 弹反窗口 200ms 不变（worldview §五:434 锁定）vs 化虚级 250ms

- **A**：化虚保持 200ms（worldview 严格）
- **B**：化虚扩到 250ms（同 woliu 瞬涡化虚级 250ms）

**默认推 B** —— 跟 woliu 瞬涡化虚级一致格调（化虚反应能力极致），且 worldview §五:434 没明确 "200ms 不能变"，仅说"极限弹反"

### #4 多点反震是否可以与护脉并存？

- **A**：可（不冲突，护脉是经脉硬化、反震是皮下震爆，物理不同层）
- **B**：不可（同时维持 qi cost 过高）
- **C**：可，但护脉经脉数 -1（化虚原可叠 2 条降为 1 条）

**默认推 A** —— 物理上不冲突，玩家自由组合多招应是流派玩法核心

### #5 暴烈色 hook 实装位置

跟 dugu / tuike 一致：

- **A**：zhenmai-v2 P1 内自行查询 PracticeLog 暴烈色比例（推荐）
- **B**：扩展 cultivation::QiColor 加 style_passive_buff fn（其他流派复用）
- **C**：等 plan-style-balance-v1 实装时统一处理

**默认推 A** —— 跟 dugu-v2 / tuike-v2 一致，每流派自行查询

### #6 熟练度公式 vs plan-skill-v1 lv 映射

本 plan 引入熟练度生长（冷却 / 弹反窗口随 skill_lv 线性变化）。但 plan-skill-v1 当前 `xp_to_next(lv) = 100 × (lv+1)²` 曲线让 lv 50 = 130050 xp，跟 100h 路径"使用 1000-5000 次 ≈ 50-100 lv"不匹配。

- **A**：本 plan 内自行用 `use_count` 替代 lv（脱离 plan-skill-v1 系统）
- **B**：调整 plan-skill-v1 zhenmai 招式专属 xp_to_next 曲线（如 `10 × (lv+1)`）
- **C**：保持 plan-skill-v1 lv 但调整本 plan 公式映射区间到 [0, 30]（实际能达到的 lv 范围）

**默认推 B** —— 跟现有 SkillSet 系统对接，但每流派可能需要不同曲线（截脉弹反 1000 次 vs 体修崩拳 100 次）。归 plan-skill-v1 vN+1 通用化或本 plan P1 自行调整

### #7 熟练度生长机制是否回填其他 v2 plan

本 plan 是首个引入"熟练度生长（冷却 + 窗口）"的 v2 流派 plan。其他已立 v2 plan：

- plan-woliu-v2 ✅ skeleton（瞬涡 5s 冷却 / 涡口 8s / 涡引 30s 等都是固定）
- plan-dugu-v2 ✅ skeleton（蚀针 3s / 侵染 8s 固定）
- plan-tuike-v2 ✅ skeleton（蜕一层 8s / 转移污染 30s 固定）

- **A**：现在批量回填三个 v2 plan（耗时但一致性好）
- **B**：留各自 P0 决策门时统一处理
- **C**：派生 plan-skill-proficiency-v1 通用 plan，所有 v2 plan 引用

**默认推 B** —— 各 v2 plan P0 启动时各自补，避免一次性批量改散乱化。但应在 reminder.md 登记此通用机制

---

## §6 进度日志

- **2026-05-06** 骨架立项，承接 plan-zhenmai-v1 ✅ finished（PR #122 commit 1ae7fd88 归档；P0 极限弹反已实装于 plan-combat-no_ui）。
  - 设计轴心：血肉派定调（HP / 经脉换 qi 免伤）+ 音论物理（worldview §P 定律 5 + cultivation-0002 §音论）+ 5 招完整规格 + **化虚专属绝脉断链**（worldview §四:319 主动 SEVERED + §P 音论 K_drain 破例：60s 反震效率 ×3，**不是免疫**，受击仍命中但反震高效）+ 反噬阶梯含 SEVERED + 瞬时痕迹（区别于其他流派的长期场）
  - 五招完整规格 7 档威力表锁定（极限弹反 / 局部中和 / 多点反震 / 护脉 / 绝脉断链）
  - **化虚做新招**（区别于 tuike 物资派"无新招仅钱包深"）—— 截脉是血肉派，化虚级用经脉换反震高效率（K_drain 破例）是 §P 音论物理一致
  - 反制化虚毒蛊师两路径：
    1. 替尸 ③ 上古伪皮 hard counter（烧物资）
    2. 截脉 ⑤ 绝脉断链选"脏真元类"反震 ×3（烧身体，永久 SEVERED 一条经脉，**仍受伤但反震高效**）
  - 专属物理边界 = 瞬时痕迹（5-10s 散尽）：皮下血雾 + 中和余响 + 断脉光痕，区别于涡流紊流场（5min）/ 毒蛊残留（30min）/ 替尸蜕落物（30min）的长期场
  - worldview 锚点对齐：§三:78 化虚天道针对 + §四:283-300 + §四:319 SEVERED + §五:432-436 + §五:464 + §六:619 + §K + §P 定律 5
  - qi_physics 锚点：等 patch P0/P3 完成后接入；R_jiemai / β=0.6 / W vs 4 攻矩阵 / SEVERED 写入 MeridianSystem 走 qi_physics 算子
  - SkillRegistry / PracticeLog / HUD / 音效 / 动画 全部底盘复用，无新建框架
  - 7 流派 v2 当前进度：涡流 ✅ / 毒蛊 ✅ / 替尸 ✅ / **截脉 ✅**（防御 3 流全部立项）/ 体修 ⬜ / 暗器 ⬜ / 阵法 ⬜（攻击 3 流剩 3 个）
  - 待补：与 plan-style-balance-v1 W/β 矩阵对齐 / 暴烈色 hook（zhenmai-v2 P1 自行查询 PracticeLog）/ plan-tribulation-v1 化虚断脉触发天道注视 / plan-multi-life-v1 SEVERED 跨周目处理 / plan-narrative-political-v1 化虚断脉求生江湖传闻 / **熟练度生长机制回填其他 v2 plan**（woliu/dugu/tuike 留各自 P0 处理 + reminder.md 登记通用机制）
- **2026-05-06 v2 修订（用户拍）**：
  - 引入**熟练度生长二维划分** —— 境界决定威力上限（K_drain / 反震点数 / 硬化抗性 / 中和兑换率）/ 熟练度决定响应速率（冷却 / 弹反窗口 / cast time）
  - 修订 §0 设计轴心加"熟练度生长（通用机制）"
  - 修订 §2 ① 极限弹反：新增熟练度维度表（弹反窗口 100→250ms，冷却 30→5s 跟 skill_lv 线性）；境界维度仅留威力相关（K_drain / 自伤 HP）
  - 修订 §2 ② 局部中和 / ③ 多点反震 / ④ 护脉 冷却字段为"按熟练度"
  - ⑤ 绝脉断链不变（化虚专属罕见，每次断一条经脉本就受限）
  - 新增 §5 #6（公式 vs plan-skill-v1 lv 映射）+ #7（是否批量回填其他 v2 plan）
  - 哲学：worldview §五:537 流派由组合涌现 + §五:506 末土后招原则物理化身——能力跟练得多有关，不绑死境界

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：5 招对应 server/agent/client 模块路径 + v1 极限弹反迁入 + 永久 SEVERED 持久化机制
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test combat::zhenmai_v2` + 测试数 / `narration-eval` 5 招 + 化虚断脉求生叙事 / WSLg 联调实录 / 反制 dugu 倒蚀实战测试
- **跨仓库核验**：server 5 招 SkillRegistry + 永久 SEVERED 写入 MeridianSystem / agent 5 招 narration + 化虚断脉江湖传闻 / client 4 HUD + 3 粒子 + 5 动画 + 4 音效
- **遗留 / 后续**：暴烈色 passive_buff 通用化（其他流派 vN+1 也需类似 hook 时提取）/ telemetry 校准（plan-style-balance-v1）/ 化虚断脉的反制路径（化虚毒蛊师反制 zhenmai 反震加成，留 dugu vN+1）/ 上古接经术残卷恢复 SEVERED 路径（plan-tsy-loot-v1 + 多周目策略）+ 接经术医者 NPC（plan-yidao-v1 🆕 实装）

- **2026-05-06 修订（用户拍：免伤无物理逻辑）**：
  - 撤销"60s 选择性免疫"的 MMO hack 机制
  - 改为 worldview §P 定律 5 音论 + §P clamp 破例物理推导：60s 内反震效率 K_drain 0.5 → 1.5（×3 倍）+ 自身受伤减少 50%
  - 受击仍正常命中（伤害走 §P 物理矩阵），化虚截脉师是赌断脉空隙引能高效率反震
  - 反制 dugu 倒蚀仍可（反吸 dugu 真元 + 受伤减半），但 dugu 倒蚀的永久标记仍生效（受全额命中）
  - SelectiveImmunity component 改名 BackfireAmplification，移除"全免疫 flag"
  - SelectiveImmunityActiveEvent 改名 BackfireAmplificationActiveEvent
