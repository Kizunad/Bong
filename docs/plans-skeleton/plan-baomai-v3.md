# Bong · plan-baomai-v3 · 骨架

体修·爆脉功法**五招完整包**：动画 / 特效 / 音效 / 伤害 / 真元消耗 / 反噬 / 客户端 UI 全流程。承接 `plan-baomai-v1` ✅ finished（PR #76 commit b0302396 归档；崩拳 P0 已实装）+ `plan-baomai-v2` ✅ active（2026-05-05，全力一击双 skill charge/release + Exhausted 虚脱期 + 完整专属 UI 蓄力球/释放雷光/虚脱灰晕 + 越级原则数值矩阵）—— v3 引入**爆脉物理**（worldview §五:402 过载撕裂 + §五:399-405 破产狂战士 + §P ρ=0.65 + §P.2 体修注入率）+ **经脉密集依赖**（手三阳全 + 任督，任一 SEVERED 多招同时废）+ **化虚专属散功**（worldview §三:187 化虚 ×5 凡躯重铸物理化身——主动烧 qi_max 50% 换 5s 全免）+ 熟练度生长二维划分（zhenmai-v2 通用机制回填）。

**世界观锚点**：`worldview.md §三:187 化虚 ×5 质变（凡躯彻底重铸）`（散功物理依据）· `§五:399-405 体修/爆脉流核心定义`（破产狂战士+零距离贴脸+过载撕裂）· `§五:402 过载撕裂物理`（赌命爆发，战后真元上限永久扣除）· `§五:466 primary axis 经脉龟裂深度+真元过载倍率`· `§六:611 沉重色`（真元浑厚下沉密度极高 / 近身爆发+ / 抗物理冲击+）· `§四:354 过载撕裂引脉裂物理`· `§四:368-372 越级矩阵`（化虚拳碾压低境的池子物理）· `§P ρ=0.65 体修异体排斥率最高`· `§P.2 α=0.3 异体侵入消耗系数`· `§K narration 沉默`

**library 锚点**：`cultivation-0003 爆脉流正法`（体修一手参考资料）· `peoples-0006 战斗流派源流` 体修源流 · `cultivation-0002 烬灰子内观笔记 §音论`（多点 vs 单点接触面物理推导，对位 zhenmai 弹反）

**前置依赖**：

- `plan-qi-physics-v1` P1 ship → ρ=0.65 异体排斥走 `qi_physics::collision::qi_collision`
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 ρ/W/β 矩阵实装（体修 ρ=0.65 / vs 涡流 W=0.8 强克）
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ + `plan-multi-style-v1` ✅
- `plan-baomai-v1` ✅ finished + `plan-baomai-v2` ✅ active（崩拳 + 全力一击双 skill 已实装，本 plan 复用并校准）
- `plan-meridian-severed-v1` 🆕 → 招式依赖经脉强约束 + SEVERED 7 类来源（含过载撕裂 OverloadTear）
- `plan-cultivation-canonical-align-v1` ✅
- `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 招的 ρ/W 数值进矩阵（体修 ρ=0.65 / vs 涡流 W=0.8 / vs 替尸 W=0.3 / vs 截脉 W=0.5）
- `plan-tribulation-v1` ⏳ → 化虚级散功触发"绝壁劫"
- `plan-narrative-political-v1` ✅ active → 化虚体修一战烧 qi_max 50% 的江湖传闻
- `plan-meridian-severed-v1` 🆕 → 焚血累积 + 散功化虚 + 全力一击 Exhausted 都可触发 SEVERED

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, contamination, qi_color }` / `cultivation::MeridianSystem` / `combat::Wounds`（HP 自伤）/ `qi_physics::ledger::QiTransfer` / `qi_physics::collision::qi_collision`（ρ=0.65）/ `SkillRegistry` + 招式 dependencies(meridian_ids) 接口（plan-meridian-severed-v1 强约束）/ `plan-baomai-v1` 崩拳 fn 复用 + `plan-baomai-v2` charge/release fn 复用
- **出料**：5 招 `BaomaiSkillId` enum 注册到 SkillRegistry（崩拳 v1 + 全力一击 v2 双 skill + 撼山 / 焚血 / 散功 v3 新增）/ `MountainShakeEvent` 🆕（撼山 AOE 震波）/ `BloodBurnEvent` 🆕（焚血激活）/ `DispersedQiEvent` 🆕（散功化虚 qi_max -50% 永久）/ `OverloadMeridianRippleEvent`（worldview §五:466 经脉龟裂深度可视）/ MeridianSeveredEvent 通过 plan-meridian-severed-v1 接管
- **共享类型**：`StyleAttack` trait（qi_physics::traits）/ `BloodBurnActive` component（焚血激活状态 + 持续 ticks + qi_multiplier）/ `BodyTranscendence` component（化虚散功 5s 凡躯重铸状态 + 全免疫 flag）/ `MeridianRippleScar` component（worldview §五:466 经脉龟裂可视化，体修专属"履历感"）
- **跨仓库契约**：
  - server: `combat::baomai_v3::*` 主实装（v1 崩拳 + v2 全力一击迁入 + v3 新 3 招）/ `schema::baomai_v3`
  - agent: `tiandao::baomai_v3_runtime`（5 招 narration + 化虚散功凡躯重铸叙事 + 焚血赌命叙事 + 撼山地震叙事 + 经脉龟裂履历叙事）
  - client: 5 动画 + 4 粒子 + 4 音效 recipe + 3 HUD 组件
- **worldview 锚点**：见头部
- **qi_physics 锚点**：所有招式 ρ=0.65 走 `qi_physics::collision::qi_collision`；撼山 AOE 走 `qi_physics::field::aoe_ground_wave` 🆕（patch P3 加）；焚血 HP→qi 转化走 `qi_physics::field::blood_burn_conversion` 🆕；散功凡躯重铸走 `qi_physics::field::body_transcendence` 🆕（patch P3 加）；**禁止 plan 内自己写注入 / AOE / 焚血公式**

---

## §0 设计轴心

- [ ] **破产狂战士定调（worldview §五:399-405）**：体修**不依赖外物**——区别于物资派（替尸钱包代价）和载体派（暗器封灵）。所有代价在**自身肉体经脉**：
  - ① 崩拳：贴脸强灌真元 + qi 大量消耗
  - ② 全力一击：全池一次性贯注 + 战后 Exhausted 虚脱（v2 已实装）
  - ③ 撼山：AOE 砸地 + qi 消耗 + 自身经脉震荡
  - ④ 焚血：**HP 自损换 qi 倍率**（worldview §五:402 用血肉换威力）
  - ⑤ 散功：**化虚级 qi_max 永久 -50%** 换 5s 全免

- [ ] **过载撕裂物理（worldview §四:354 + §五:402）**：
  ```
  正常流量上限：引气 5/s（worldview §四:354）
  过载流量：强行 ×3-5 倍（崩拳 / 全力一击）
  代价：经脉裂痕（contam 累积 + 长期 MICRO_TEAR/TORN/SEVERED）
  战后修复：长时间静坐 + 凝脉散内服 + 灵草外敷
  worldview §五:402 "战后真元上限永久扣除/临时冻结"
  ```
  
  长期体修 = 经脉千疮百孔（`MeridianRippleScar` component 累积）+ qi_max 极高的悖论。worldview §六:611 "真元浑厚下沉密度极高" 物理化身

- [ ] **经脉密集依赖（plan-meridian-severed-v1 §3 强约束）**：
  - ① 崩拳：手三阳全（任一断 → 威力 ×0.5）
  - ② 全力一击：任督 + 手三阳全（任督断 → 废 / 手三阳任一断 → 池子充能 ×0.5）
  - ③ 撼山：足三阳全（震波传地）+ 手三阳
  - ④ 焚血：足三阴 LR（肝主血）+ 任督
  - ⑤ 散功：所有经脉（化虚级"凡躯重铸"动用全身，任一已 SEVERED → 散功失败）
  
  体修是 7 流派中**经脉依赖最密集**的——单条 SEVERED 影响 2-3 招，全身 SEVERED 多 → 体修全废。这是 worldview §五:402 "战后长时间休养经脉"的物理体现

- [ ] **化虚级散功（worldview §三:187 ×5 凡躯重铸）**：化虚体修不做新机制（如涡心紊流死区 / 倒蚀引爆 / 绝脉断链），而是**主动烧 qi_max 50% 换 5s 全免**——化虚 ×5 质变的物理化身：凡躯一次性"重铸"+ 池子永久缩水 50%。
  - 化虚 qi_max 10700 ×50% = 5350 永久损失
  - 5s 内免疫一切伤害（含 dugu 永久标记 / woliu 紊流场抽干 / zhenmai 反震 / 一切物理）
  - 5s 内所有招式 cooldown 清零（极限连击窗口）
  - **不可逆**（除非重新突破或上古遗物）
  - 跟其他化虚专属对比：
    - 涡心：长期场（紊流死区）
    - 倒蚀：远程清算（积累 → 引爆）
    - **散功：自损 50% 池子换瞬间无敌**（最 PvP 直接的化虚招）
  - 反制 dugu 倒蚀的第三路径（与 tuike ③ 上古伪皮 / zhenmai ⑤ 绝脉断链并列）

- [ ] **专属物理边界 = 经脉龟裂尾迹（永久身体记录）**：跟其他流派最大区别：
  - 涡流：紊流场 5min 散尽（外部环境）
  - 毒蛊：脏真元残留 30min（受害者侧）
  - 替尸：蜕落物 30min（地面物）
  - 截脉：瞬时 5-10s 痕迹
  - **体修：经脉龟裂可视化，永久记录在自身 inspect**——其他玩家高境 inspect 看到 "这人是老体修"（worldview §五:466 经脉龟裂深度 primary axis）
  
  这是体修"履历感"的物理化身——长期体修无法隐藏身份（worldview §五 末土后招原则有限——经脉龟裂深刻于身）

- [ ] **熟练度生长（v2 通用机制回填，zhenmai-v2 首发）**：
  ```
  境界 = 威力上限（qi_current × 倍率 / qi_max -50% 量级 / AOE 半径）
  熟练度 = 响应速率（cooldown / charge 速率 / 焚血效率）

  熟练度生长曲线（线性递变）：
    ① 崩拳 cooldown: 3s → 0.5s（lv 0→100）
    ② 全力一击 charge 速率: 50/tick → 200/tick
    ② Exhausted 时长按 qi_committed 比例 → 熟练度高时缩短 30%
    ③ 撼山 cooldown: 30s → 10s
    ④ 焚血 cooldown: 60s → 20s
    ⑤ 散功重铸时长: 5s → 8s（lv 100 化虚级）
  ```

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：5 招数值表锁定 + ① 崩拳 v1 数值校准 + ② 全力一击 v2 数值校准 + 新 3 招（撼山/焚血/散功）数值表 + 经脉依赖清单（plan-meridian-severed-v1 §3 强约束）+ 熟练度生长曲线 + §5 五决策门收口 + qi_physics 接入面定稿 + 与 plan-style-balance-v1 ρ/W 矩阵对齐 | 数值矩阵 + 物理公式落 plan §2 |
| **P1** ⬜ | server `combat::baomai_v3::*` 5 招 logic（v1 崩拳 + v2 全力一击迁入 + v3 撼山/焚血/散功 新增）+ 过载撕裂物理 + 经脉依赖检查（走 plan-meridian-severed-v1）+ 焚血 HP→qi 转化 + 散功凡躯重铸 5s 全免 + qi_max 永久 -50% 持久化 + qi_physics 算子调用 + ≥120 单测 | `cargo test combat::baomai_v3` 全过 / 守恒断言 / 散功 qi_max 永久 -50% 跨 server restart 测试 / `grep -rcE '#\[test\]' server/src/combat/baomai_v3/` ≥ 120 |
| **P2** ⬜ | client 5 动画（崩拳 / 全力一击双 v2 已 ✅ / 撼山砸地姿态 / 焚血割腕滴血动作 / 散功化虚级凡躯震颤光柱）+ 4 粒子（GROUND_WAVE_DUST / BLOOD_BURN_CRIMSON / BODY_TRANSCENDENCE_PILLAR / MERIDIAN_RIPPLE_SCAR 经脉龟裂可视）+ 3 HUD 组件（BloodBurnRatioHud / BodyTranscendenceTimerHud / MeridianRippleScarHud） | render_animation.py 验证 / WSLg 实跑 5 招视觉确认 |
| **P3** ⬜ | 4 音效 recipe（mountain_shake_rumble / blood_burn_sizzle / transcendence_thunder / meridian_crack）+ agent 5 招 narration template + 化虚散功凡躯重铸叙事 + 焚血赌命叙事 + 经脉龟裂履历叙事 + 化虚散功触发绝壁劫预兆 | narration-eval ✅ 5 招 + 化虚级孤注叙事 全过古意检测 |
| **P4** ⬜ | PVP telemetry 校准 / 沉重色 hook（PracticeLog → QiColor 沉重色累积演化）+ vs 7 流派对位（特别 vs 涡流 W=0.8 强克 / vs 替尸 W=0.3 / vs 截脉 W=0.5）+ 化虚散功反制 dugu 倒蚀实战测试（与 tuike + zhenmai 反制路径对照）+ 长期体修经脉龟裂履历演化测试 | 7 流派 4×3 攻防对位体修通过 / 化虚级孤注实战 / 沉重色长期累积演化 |

**P0 决策门**：完成前 §5 五决策必须有答案。

---

## §2 五招完整规格

### ① 崩拳（v1 ✅ finished，v3 校准 + 熟练度生长 + 经脉依赖）

worldview §五:402 + §P ρ=0.65。零距离贴脸强灌真元，过载倍率 ×1.5。

**境界维度（威力 = qi_invest × 倍率）**：

| 境界 | qi 投入比例 | 过载倍率 | 实际伤害（基线 qi_max × 0.4 × 1.5）|
|---|---|---|---|
| 醒灵 | 40% | ×1.0（无过载，qi_max 太小） | 4 |
| 引气 | 40% | ×1.2 | 19 |
| 凝脉 | 40% | ×1.5 | 90 |
| 固元 | 40% | ×1.5 | 324 |
| 通灵 | 40% | ×1.5 | 1260 |
| 半步化虚 | 40% | ×1.6 | 4500 |
| 化虚 | 40% | ×1.6 | 6850 |

**熟练度维度（响应速率）**：

| skill_lv | 冷却 | 自伤 contam |
|---|---|---|
| 0 | 3s | +0.05 |
| 50 | 1.75s | +0.025 |
| 100 | 0.5s | +0.0 |

**依赖经脉**（强约束）：手三阳全（LI / SI / TE）—— 任一 SEVERED → 威力 ×0.5
**worldview 锚**：§五:402 + §P 定律 1 + §六:611 沉重色

### ② 全力一击（v2 ✅ active，v3 集成 + 校准）

worldview §四:381-391 全力一击物理 + plan-baomai-v2 已实装 charge/release 双 skill + Exhausted。

**v2 已实装机制**（保留）：
- charge：每 tick 蓄力 50 qi 入蓄力球（client 蓄力球 UI）
- release：根据池子比例释放（10%/40%/100% 三档）
- Exhausted：按 qi_committed 比例的虚脱期（防御 -50% / 真元回复 -50%）
- 客户端：蓄力球 / 释放雷光 / 虚脱灰晕完整 UI（v2 已 ✅）

**v3 校准**：

| 境界 | charge 100% qi_max 时伤害 | Exhausted 时长 |
|---|---|---|
| 醒灵 | 10 | 30s |
| 引气 | 40 | 30s |
| 凝脉 | 150 | 30s |
| 固元 | 540 | 60s |
| 通灵 | 2100 | 90s |
| 半步化虚 | 5350 | 120s |
| 化虚 | 10700 | 180s |

**熟练度维度**：
- charge 速率: 50/tick → 200/tick (lv 0→100)
- Exhausted 时长按 qi_committed × (1 - lv/200) 缩短

**依赖经脉**：任督 + 手三阳全 —— 任督断 → 全力一击废
**worldview 锚**：§四:381-391 + plan-baomai-v2 §1 已正典

### ③ 撼山（v3 新增，AOE 砸地震波）

worldview §五 体修不只是单点贴脸，AOE 砸地是体修控场招（worldview §四 距离衰减针对飞行真元，但震波传地不属飞行物——同 woliu 涡口逻辑）。

| 境界 | qi 消耗 | AOE 半径 | 震波伤害 |
|---|---|---|---|
| 醒灵 | 25 | 3 格 | 5 |
| 引气 | 25 | 4 格 | 12 |
| 凝脉 | 30 | 5 格 | 35 |
| 固元 | 35 | 6 格 | 90 |
| 通灵 | 40 | 7 格 | 220 |
| 半步化虚 | 45 | 9 格 | 480 |
| 化虚 | 50 | 10 格 | 850 |

**机制**：双拳/单足砸地 → 真元沿地表传导 N 格 → AOE 内目标受震波伤害 + 击退 0.5-1 格 + 0.5s 失衡（不能 cast）

**熟练度维度**：cooldown 30s → 10s（lv 0→100）

**依赖经脉**：足三阳全（震波传地）+ 手三阳（双拳砸地）
**冷却**：30-10s（按熟练度）
**worldview 锚**：§五 体修控场扩展 + §P 距离衰减不针对环境振动

### ④ 焚血（v3 新增，HP 自损换 qi 倍率）

worldview §五:402 体修代价物理化身——**用血肉换爆发**。

| 境界 | HP 烧 → qi 倍率 | 持续 |
|---|---|---|
| 醒灵 | 烧 10 HP → ×1.2 | 10s |
| 引气 | 烧 20 HP → ×1.5 | 15s |
| 凝脉 | 烧 50 HP → ×2.0 | 20s |
| 固元 | 烧 100 HP → ×2.5 | 25s |
| 通灵 | 烧 200 HP → ×3.0 | 30s |
| 半步化虚 | 烧 250 HP → ×3.5 | 30s |
| 化虚 | 烧 300 HP → ×4.0 | 30s |

**机制**：cast 时玩家选择烧多少 HP（弹窗滑块）→ 触发 BloodBurnActive component → 期间所有招式 qi 投入 ×N 倍率（不只是攻击伤害，也包括 ② 充能速率 + ③ 撼山 / ④ 自身）

**反噬**：
- HP 不够烧（< 阈值）→ Reject「血气不足」
- 烧到 HP < 10% → 强制结束焚血 + 玩家进入垂死状态
- 焚血结束后 contam +5% / 持续 5min（血气燃尽副作用）

**熟练度维度**：cooldown 60s → 20s（lv 0→100）+ HP 烧→qi 倍率公式效率随熟练度小幅提升（lv 100 ×1.05）

**依赖经脉**：足三阴 LR（肝主血）+ 任督
**worldview 锚**：§五:402 战后真元上限永久扣除 + §四 部位伤口物理

### ⑤ 散功 — 化虚专属（凡躯重铸 5s 全免，qi_max 永久 -50%）

worldview §三:187 化虚 ×5 质变物理化身。**化虚专属**——通灵以下 cast 仅触发 5% qi_max 损失但无重铸效果（HUD「凡躯不应」）。

| 施法者境界 | 凡躯重铸时长 | qi_max 永久损失 | 5s 内冷却清零 |
|---|---|---|---|
| 醒灵-通灵 | — | -5%（仅惩罚，无效果）| — / **HUD「凡躯不应」** |
| 半步化虚 | 3s | -40% | 是 |
| 化虚 | 5s（lv 100 可达 8s）| -50% | 是 |

**机制**：
- 主动 cast → 一次性烧 qi_max × 50%（化虚级）→ 触发 BodyTranscendence component
- BodyTranscendence 期间 5s（lv 100 化虚级 8s）：
  - **免疫一切伤害**（含 dugu 永久标记 / woliu 紊流场 / zhenmai 反震 / 物理 / 真元 / 阵法 全免）
  - **所有招式 cooldown 清零**（极限连击窗口）
  - **可继续 cast 任何招式**（包括 ② 全力一击不进入 Exhausted）
- 5s 后：BodyTranscendence 移除 → qi_max 永久 -50% 写入 cultivation::Cultivation
- 不可逆（worldview §三:80 维护成本极高 + 重新突破需走完突破流程）
- 化虚连续 cast 3 次散功（30 days in-game 内）→ 触发"绝壁劫"（强度 ×1.5，跟涡心 / 倒蚀 / 绝脉断链一致格调）

**叙事意象**：化虚体修在生死关头，全身经脉骤然爆发金光 → 5s 内对手攻击如击虚空，他自己拳脚如雷霆 → 5s 后金光散尽，整个人衰老 10 岁，池子永久缩水一半。这是 worldview §三:187 "凡躯彻底重铸" 的物理化身——重铸不是变强，是用过去的本钱赌一次

**反制 dugu 倒蚀的第三路径**（与 tuike ③ 上古伪皮 / zhenmai ⑤ 绝脉断链并列）：
- 化虚毒蛊师倒蚀引爆永久 qi_max 衰减
- 化虚体修 ⑤ 散功 → 5s 内全免疫（含 dugu 永久标记）
- 代价：qi_max 永久 -50%
- 跟其他反制路径对比：
  - tuike ③：烧物资（一件上古级伪皮）
  - zhenmai ⑤：烧身体（永久 SEVERED 一条经脉）
  - **baomai ⑤**：烧池子（永久 -50% qi_max）

**依赖经脉**：所有 20 经脉（任一已 SEVERED → 散功失败 + 5% qi_max 损失，无重铸）
**worldview 锚**：§三:187 + §五:402 + §三:78 化虚天道针对

---

## §3 数据契约

```
server/src/combat/baomai_v3/
├── mod.rs              — Plugin + register_skills (集成 v1 崩拳 + v2 全力一击 + v3 新 3 招)
├── skills.rs           — BaomaiSkillId enum (BengQuan/Charge/Release/
│                                              MountainShake/BloodBurn/Disperse)
│                        + 5 resolve_fn（BengQuan/Charge/Release v1+v2 复用）
├── state.rs            — BloodBurnActive component (qi_multiplier + duration_ticks)
│                        + BodyTranscendence component (5s 全免疫 + cooldown 清零 flag)
│                        + MeridianRippleScar component (worldview §五:466 经脉龟裂
│                                                        累积可视，体修专属履历)
├── tick.rs             — blood_burn_tick / body_transcendence_tick / 
│                        meridian_ripple_accumulate_tick (长期体修自动累积)
├── physics.rs          — 过载撕裂 + ρ=0.65 注入 + AOE 震波传地 +
│                        焚血 HP→qi 转化 + 散功凡躯重铸 + qi_max 永久 -50% 写入
└── events.rs           — BurstMeridianEvent (扩展 v1) /
                          FullPowerAttackIntent (扩展 v2) /
                          MountainShakeEvent / BloodBurnEvent /
                          DispersedQiEvent / OverloadMeridianRippleEvent

server/src/schema/baomai_v3.rs  — IPC schema 5 招 + qi_max 永久 -50% + 焚血 + 凡躯重铸 payload

agent/packages/schema/src/baomai_v3.ts  — TypeBox 双端
agent/packages/tiandao/src/baomai_v3_runtime.ts  — 5 招 narration +
                                                   化虚散功凡躯重铸叙事 +
                                                   焚血赌命叙事 +
                                                   经脉龟裂履历叙事 +
                                                   化虚散功触发绝壁劫预兆

client/src/main/java/.../combat/baomai/v3/
├── BaomaiV3AnimationPlayer.java          — 5 动画（崩拳 v1 + 全力一击双 v2 +
                                              撼山砸地 + 焚血割腕 + 散功凡躯震颤）
├── GroundWaveDustParticle.java           — 撼山砸地震波 + 尘土
├── BloodBurnCrimsonParticle.java         — 焚血红雾 + 血珠飞溅
├── BodyTranscendencePillarParticle.java  — 散功化虚级凡躯震颤光柱（金光）
├── MeridianRippleScarParticle.java       — 经脉龟裂可视（inspect 模式下显示）
├── BloodBurnRatioHud.java                — 焚血当前 HP 比例 + 持续倒计
├── BodyTranscendenceTimerHud.java        — 散功 5s 凡躯重铸倒计 + 全免提示
└── MeridianRippleScarHud.java            — 经脉龟裂履历显示（inspect 经脉图扩展）

client/src/main/resources/assets/bong/
├── player_animation/baomai_mountain_shake.json
├── player_animation/baomai_blood_burn.json
├── player_animation/baomai_disperse.json
└── audio_recipes/mountain_shake_rumble.json + blood_burn_sizzle.json +
                  transcendence_thunder.json + meridian_crack.json
```

**SkillRegistry 注册（与 plan-meridian-severed-v1 §3 强约束对齐）**：

```rust
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(SkillBuilder::new("baomai.beng_quan")  // v1 崩拳
        .resolve_fn(cast_beng_quan)
        .dependencies(vec![MeridianId::LI, MeridianId::SI, MeridianId::TE])
        .build());
    
    registry.register(SkillBuilder::new("baomai.full_power_charge")  // v2 充能
        .resolve_fn(cast_charge)
        .dependencies(vec![MeridianId::REN, MeridianId::DU,
                          MeridianId::LI, MeridianId::SI, MeridianId::TE])
        .build());
    
    registry.register(SkillBuilder::new("baomai.full_power_release")  // v2 释放
        .resolve_fn(cast_release)
        .dependencies(vec![MeridianId::REN, MeridianId::DU,
                          MeridianId::LI, MeridianId::SI, MeridianId::TE])
        .build());
    
    registry.register(SkillBuilder::new("baomai.mountain_shake")  // v3 撼山
        .resolve_fn(cast_mountain_shake)
        .dependencies(vec![MeridianId::ST, MeridianId::BL, MeridianId::GB,
                          MeridianId::LI, MeridianId::SI, MeridianId::TE])
        .build());
    
    registry.register(SkillBuilder::new("baomai.blood_burn")  // v3 焚血
        .resolve_fn(cast_blood_burn)
        .dependencies(vec![MeridianId::LR, MeridianId::REN, MeridianId::DU])
        .build());
    
    registry.register(SkillBuilder::new("baomai.disperse")  // v3 散功化虚专属
        .resolve_fn(cast_disperse)
        .dependencies(MeridianId::all_20())  // 所有经脉
        .build());
}
```

**PracticeLog 接入**：

```rust
emit SkillXpGain {
    char: caster,
    skill: SkillId::Baomai,
    amount: per_skill_amount(skill_kind),  // beng 1 / charge 1 / release 2 /
                                           // mountain 2 / blood_burn 2 / disperse 5
    source: XpGainSource::Action {
        plan: "baomai_v3",
        action: skill_kind.as_str(),
    }
}
```

PracticeLog 累积驱动 QiColor **沉重色**（worldview §六:611）演化。沉重色加成：worldview §六:611 "近身爆发+ / 抗物理冲击+" → 体修崩拳威力 ×1.05 + 受物理伤害 ×0.95（沉重色 ≥ 30% 后）

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| 动画 | `bong:baomai_mountain_shake` | 新建 JSON | P2 | 双拳/单足砸地姿态，priority 1500 |
| 动画 | `bong:baomai_blood_burn` | 新建 JSON | P2 | 割腕滴血动作 + 全身红光，priority 1200 |
| 动画 | `bong:baomai_disperse` | 新建 JSON | P2 | 化虚级凡躯震颤光柱 + 全身金光，priority 1800 |
| 粒子 | `GROUND_WAVE_DUST` ParticleType + Player | 新建 | P2 | 撼山砸地尘土 + 震波传地纹路 |
| 粒子 | `BLOOD_BURN_CRIMSON` ParticleType + Player | 新建 | P2 | 焚血红雾 + 血珠飞溅 |
| 粒子 | `BODY_TRANSCENDENCE_PILLAR` ParticleType + Player | 新建 | P2 | 散功凡躯震颤光柱（金光柱状）|
| 粒子 | `MERIDIAN_RIPPLE_SCAR` ParticleType + Player | 新建 | P2 | 经脉龟裂可视（inspect 模式下显示，体修专属履历） |
| 音效 | `mountain_shake_rumble` | recipe 新建 | P3 | layers: `[{ sound: "entity.generic.explode", pitch: 0.5, volume: 0.7 }, { sound: "block.stone.break", pitch: 0.6, volume: 0.5, delay_ticks: 2 }]`（地震 + 石头碎裂）|
| 音效 | `blood_burn_sizzle` | recipe 新建 | P3 | layers: `[{ sound: "entity.player.hurt", pitch: 0.8, volume: 0.6 }, { sound: "block.fire.ambient", pitch: 1.5, volume: 0.4, delay_ticks: 1 }]`（自伤痛喊 + 燃烧嘶声）|
| 音效 | `transcendence_thunder` | recipe 新建 | P3 | layers: `[{ sound: "entity.lightning_bolt.thunder", pitch: 1.3, volume: 0.8 }]`（化虚级凡躯重铸雷音）|
| 音效 | `meridian_crack` | recipe 新建 | P3 | layers: `[{ sound: "block.bone_block.break", pitch: 0.7, volume: 0.6 }]`（经脉龟裂细微声）|
| HUD | `BloodBurnRatioHud` | 新建 | P2 | 焚血当前 HP 比例 + 持续倒计 + qi 倍率显示 |
| HUD | `BodyTranscendenceTimerHud` | 新建 | P2 | 散功 5s 凡躯重铸倒计 + 全免提示 + qi_max 永久损失警示 |
| HUD | `MeridianRippleScarHud` | 新建 | P2 | 经脉龟裂履历（inspect 经脉图扩展，体修专属，他人 inspect 也可见）|

---

## §4.5 P1 测试矩阵（饱和化）

下限 **120 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_beng_quan` (v1 集成) | 7 境界过载倍率 + 熟练度生长冷却 + 经脉依赖检查 + 自伤 contam | 18 |
| `cast_charge / cast_release` (v2 集成) | charge 速率熟练度变化 + release 池子比例 + Exhausted 时长熟练度变化 + 任督依赖 | 22 |
| `cast_mountain_shake` (v3) | 7 境界半径 + 震波传地 + 击退 + 失衡 + 足三阳依赖 + cooldown 熟练度 | 18 |
| `cast_blood_burn` (v3) | 7 境界 HP→qi 倍率 + HP 不足 reject + 烧到 < 10% 强制结束 + 焚血结束 contam +5% + LR 依赖 | 20 |
| `cast_disperse` (v3 化虚专属) | 化虚专属判定 + 通灵以下「凡躯不应」+ qi_max 永久 -50% 持久化（跨 server restart）+ 5s 全免 + cooldown 清零 + 重铸期间所有招式 cast 测试 + 反制 dugu 倒蚀实战 + 30 days 内连续 3 次触发绝壁劫 | 25 |
| `meridian_ripple_scar_accumulate` | 长期体修自动累积可视化 + inspect 他人可见 + 跨 server restart 持久化 | 8 |
| `meridian_severed_baomai` | plan-meridian-severed-v1 7 类来源中 baomai 接入（OverloadTear 过载撕裂 SEVERED 判定 + BackfireOverload 累积超阈值 SEVERED） | 9 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/combat/baomai_v3/` ≥ 120。守恒断言：所有 qi_invest 走 `qi_physics::ledger::QiTransfer`。

---

## §5 开放问题 / 决策门

### #1 化虚级散功 qi_max 永久 -50% 是否过苛

- **A**：保留 50%（worldview §三:187 ×5 质变 = 凡躯彻底重铸物理化身）
- **B**：调整 30%（避免化虚体修一战变废）
- **C**：阶梯式（半步化虚 -40% / 化虚 -50% / 化虚 lv 100 -45%）

**默认推 A** —— 化虚级孤注本就该极端（跟涡心紊流劫 / 倒蚀绝壁劫 / 绝脉断链 SEVERED 一致格调）

### #2 焚血 HP 烧到 < 10% 是否强制结束

- **A**：强制结束 + 玩家进入垂死状态（防玩家自杀式焚血）
- **B**：玩家自由选择（worldview §五:402 体修就是"赌命"，玩家应有自由）
- **C**：< 10% 进入垂死但仍可烧（双层风险：垂死状态 + 真死亡风险）

**默认推 A** —— 防作死。worldview §五:402 是"赌命爆发"不是"自杀爆发"

### #3 经脉龟裂可视化是 passive 累积还是手动触发

- **A**：passive 累积（每次过载累积一点，长期体修千疮百孔自然显现）
- **B**：手动触发（玩家 cast 时显示）
- **C**：A + B 组合（passive 累积 + 战斗中触发显示）

**默认推 A** —— worldview §五:466 经脉龟裂深度是 primary axis，应该是 passive 状态而非主动 trigger

### #4 散功反制 dugu 倒蚀的"全免疫"是否包括所有 component

- **A**：全免疫（含 dugu 永久标记 / woliu 紊流场 / zhenmai 反震 / 物理 / 真元）
- **B**：仅免疫直接伤害（永久标记仍生效，只是 5s 内不发作）
- **C**：玩家 cast 时选 1 类免疫（同 zhenmai ⑤）

**默认推 A** —— 化虚级孤注就是"5s 内绝对无敌"，简洁有力。代价 -50% qi_max 已极重

### #5 沉重色 hook 实装位置

跟其他 v2 流派一致：

- **A**：本 plan baomai-v3 P1 内自行查询 PracticeLog 沉重色比例（推荐）
- **B**：扩展 cultivation::QiColor 加 style_passive_buff fn（其他流派复用）
- **C**：等 plan-style-balance-v1 实装时统一处理

**默认推 A** —— 跟 dugu / tuike / zhenmai 一致

---

## §6 进度日志

- **2026-05-06** 骨架立项，承接 plan-baomai-v1 ✅ finished（PR #76 崩拳 P0）+ plan-baomai-v2 ✅ active（全力一击双 skill + Exhausted + UI 完整）。
  - 设计轴心：破产狂战士定调（不依赖外物，所有代价在自身肉体经脉）+ 过载撕裂物理（worldview §四:354 + §五:402）+ 经脉密集依赖（手三阳全 + 任督，7 流派最密集）+ **化虚专属散功**（worldview §三:187 ×5 凡躯重铸：烧 qi_max 50% 换 5s 全免）+ 经脉龟裂尾迹（永久身体记录履历感）+ 熟练度生长二维划分（zhenmai-v2 通用机制回填）
  - 五招完整规格 7 档威力表锁定（崩拳 v1 + 全力一击 v2 + 撼山 / 焚血 / 散功 v3 新增）
  - **化虚做新机制**（同 woliu/dugu/zhenmai 思路，区别 tuike 物资派"无新招"）—— 体修化虚级用 -50% qi_max 换瞬间无敌
  - **反制化虚 dugu 倒蚀第三路径**：tuike ③ 烧物资 / zhenmai ⑤ 烧身体（SEVERED）/ **baomai ⑤ 烧池子（-50% qi_max）**——三种 hard counter 各有取舍
  - 经脉依赖严格遵循 plan-meridian-severed-v1 §3 强约束（每招 .with_dependencies(...) 声明）
  - 反噬阶梯继承 worldview §四 4 档损伤 + plan-meridian-severed-v1 通用 SEVERED（OverloadTear 过载撕裂 + BackfireOverload 累积）
  - worldview 锚点对齐：§三:78 + §三:187 + §四:354 + §四:368-372 + §五:399-405 + §五:466 + §六:611 + §K + §P ρ=0.65
  - qi_physics 锚点：等 patch P0/P3 完成后接入；ρ=0.65 + AOE 震波 + 焚血 HP→qi 转化 + 散功凡躯重铸全走 qi_physics 算子
  - SkillRegistry / PracticeLog / HUD / 音效 / 动画 全部底盘复用
  - 7 流派 v2 当前进度：涡流 ✅ / 毒蛊 ✅ / 替尸 ✅ / 截脉 ✅ / **体修 ✅**（防御 3 流全 + 攻击 1 流立项）/ 暗器 ⬜ / 阵法 ⬜
  - 待补：与 plan-style-balance-v1 ρ/W 矩阵对齐 / 沉重色 hook（baomai-v3 P1 自行查询 PracticeLog）/ plan-tribulation-v1 化虚散功触发绝壁劫 / plan-multi-life-v1 qi_max -50% 跨周目处理（应不继承）/ plan-narrative-political-v1 化虚散功江湖传闻 / plan-meridian-severed-v1 OverloadTear 来源接入

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：5 招对应 server/agent/client 模块路径 + v1 崩拳 + v2 全力一击迁入 + qi_max 永久 -50% 持久化 + 经脉龟裂履历可视
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test combat::baomai_v3` + 测试数 / `narration-eval` 5 招 + 化虚散功孤注叙事 / WSLg 联调实录 / 反制 dugu 倒蚀实战测试
- **跨仓库核验**：server 5 招 SkillRegistry + qi_max 永久减少持久化 / agent 5 招 narration + 化虚散功江湖传闻 / client 3 HUD + 4 粒子 + 5 动画 + 4 音效 / plan-meridian-severed-v1 OverloadTear/BackfireOverload 来源接入
- **遗留 / 后续**：沉重色 passive_buff 通用化（其他 v2 plan 也需类似 hook 时提取）/ telemetry 校准（plan-style-balance-v1）/ 化虚散功反制 dugu 的平衡（化虚毒蛊师 vs 化虚体修对位测试）/ 经脉龟裂履历的 inspect UX（多周目重生后是否清空，跟 plan-multi-life-v1 联调）
