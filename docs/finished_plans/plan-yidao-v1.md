# Bong · plan-yidao-v1

医道功法**5 招完整包 + 支援流派身份**：接经术 / 排异加速 / 急救 / 续命术 / **群体接经 化虚专属**。**跟 7 战斗流派平行的支援流派**——worldview §六:617 已锚定平和色 + 医道流派定义。引入**支援流派身份认知**（PvP 中可被招募 / 雇佣 / 结契，区别于战斗流派"独行 / 对抗"模式）+ **医者 NPC 行为 AI**（big-brain Utility AI 节点，跟现有僵尸 NPC 共框架）+ **医患信誉度系统**（长期医患关系 → 平和色养成加速 + 续命丹效果增强）+ **接经术统一接入**（plan-meridian-severed-v1 主路径）+ **续命术物理推导**（worldview §十二:1043-1048 续命存在但有代价：业力 / 境界 / qi_max 上限）+ **5 招完整规格**（接经术 / 排异加速 / 急救 / 续命术 / 群体接经）+ **化虚专属群体接经**（worldview §三:187 化虚 ×5 凡躯重铸 + 平和色物理推导：化虚医者一次治愈 N 人 SEVERED，每人代价 -2% qi_max + 业力累积；**不是 MMO 式 AOE 治疗**，是真实 N 个独立接经手术，性能成本和业力代价都按 N 累加）+ **熟练度生长二维划分**（境界=治疗上限 / mastery=接经速度 / 排异效率 / 业力减免），无境界 gate 只有威力门坎。

**世界观锚点**：`worldview.md §六:617 医道·平和色`（真元温和无攻击性 + 针灸通经络效率+ + 疗他人时排异成本- + 真元几乎无杀伤性） · `§六:613 温润色 vs 平和色区分`（炼丹师走自疗主轴 / 医道走疗他人主轴，可叠加） · `§十二:1043-1048 续命物理`（续命存在但有代价：业力 / 境界 / qi_max 上限——续命丹 / 夺舍 / 坍缩渊深潜换寿） · `§十一 灵龛守护与社交`（医者 NPC 信誉度，长期医患关系作为社交基底） · `§五:537 流派由组合涌现`（医道是支援流派而非攻击流派，定调与 7 战斗流派平行） · `§三:187 化虚 ×5 凡躯重铸`（化虚级群体接经物理推导前提） · `§K narration 沉默`

**library 锚点**：`peoples-0006 战斗流派源流`（攻击三 / 防御三 各自描述时医道作"支援"提及）· `cultivation-0002 烬灰子内观笔记 §针灸论`（待 yidao-v1 P0 期间登记 — 现尚无独立医道笔记）· `ecology-0002 末法药材十七种`（医者药材源 + 与 plan-alchemy-v1 续命丹原料对齐）· `ecology-0004 灵物磨损笔记`（医道载体——针 / 灸 / 汤药——磨损共源）· `peoples-0007 医者列传`（待 yidao-v1 P0 期间立 library 馆藏，预设 3-5 位医者人物）

**前置依赖**：

- `plan-meridian-severed-v1` 🆕 active → SEVERED component + Skill::dependencies + 接经术目标接入
- `plan-alchemy-v1` ✅ → 续命丹（worldview §十二:1043 续命路径）+ 自疗丹 + 排异丹 → 医者用丹接入
- `plan-social-v1` ✅ → 医者 NPC 信誉度 + 灵龛归属（医者诊所走灵龛系统）
- `plan-multi-style-v1` ✅ → 平和色 PracticeLog（"治疗"action 加平和色 PracticeLog vector 分量）
- `plan-npc-ai-v1` ✅ → big-brain Utility AI 节点（医者 NPC 行为 AI 复用框架）
- `plan-cultivation-canonical-align-v1` ✅ → Realm + 经脉拓扑（接经术目标读 MeridianSystem）
- `plan-skill-v1` ✅ + `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅
- `plan-qi-physics-v1` P1 ship → 真元逆逸散 / 排异 / 续命走 `qi_physics::healing::*` 🆕（patch P3 加）
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 ρ 矩阵（医道 ρ=0.05 最低排斥率，因平和色异种相容性最高）+ healing 算子
- `plan-tribulation-v1` ⏳ → 续命 / 群体接经业力累积接入

**反向被依赖**：

- `plan-meridian-severed-v1` 🆕 → 主接经术服务路径（医道 = SEVERED 恢复主路径，备选 PvE = 上古接经术残卷 plan-tsy-loot-v1）
- `plan-style-balance-v1` 🆕 → 医道 ρ=0.05 + 治疗效率矩阵进表
- `plan-narrative-political-v1` ✅ active → 医患关系 + 化虚医者业力江湖传闻
- `plan-multi-life-v1` ⏳ → 跨周目业力 / 平和色继承
- `plan-anqi-v2` 🆕 → 凝魂 / 破甲注射 = 暗器派 hard counter 医道路径（医者补脉时段被截杀）
- `plan-yidao-v2` 占位 → 亚流派扩展（毒手医 / 兽医 / 道伥医，留 v2）

**反向被建议联动（非依赖）**：

- 所有玩家：任何角色都可能寻医（医道是基础公共服务）
- `plan-shelflife-v1` ✅：医者药材腐烂走 shelflife（已 finished）
- `plan-botany-v2` ✅：医者药材种植走 botany（已 finished）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation` / `cultivation::MeridianSystem` / `qi_physics::healing::*`（治疗、排异、续命）/ `qi_physics::ledger::QiTransfer`（治疗真元守恒律——医者付出，患者获得）/ `combat::Wounds` / `combat::Contamination` / `combat::lifecycle::Lifecycle`（急救救濒死患者）/ `meridian_severed::MeridianSeveredPermanent`（接经术目标）/ `alchemy::PillRegistry`（续命丹 + 自疗丹 + 排异丹）/ `social::Reputation`（医者信誉度）/ `npc::big_brain::*`（医者 NPC 行为 AI）/ `multi_style::PracticeLog`（平和色养成）/ `SkillRegistry` / `SkillSet` / `Casting` / `Realm`
- **出料**：5 招 `YidaoSkillId` enum 注册到 SkillRegistry / `MeridianHealEvent` 🆕（接经术成功）/ `ContamPurgeEvent` 🆕（排异加速）/ `EmergencyResuscitateEvent` 🆕（急救）/ `LifeExtensionEvent` 🆕（续命术 + 业力 + qi_max -% 副作用）/ `MassHealEvent` 🆕（化虚群体接经）/ `KarmaAccumulationEvent` 🆕（续命 / 群体接经业力累积）/ `MedicalContractEvent` 🆕（医患结契长期关系）/ `HealerNpcAiState` server-data（医者 NPC AI 状态推送给 client）
- **共享类型**：`HealerProfile` component（医者身份 + 信誉度 + 平和色 mastery）/ `MedicalContract` component（玩家 ↔ 医者 NPC 长期关系，跟 plan-social-v1 Reputation 共享底盘）/ `KarmaCounter` component（业力累积，续命 / 群体接经驱动）/ `HealingMastery` component（5 招各自 mastery 0-100）
- **跨仓库契约**：
  - server: `combat::yidao::*` 主实装（注意：医道在 combat/ 但功法非"对抗"，是"治疗 / 修复"——以 combat 子系统挂载因为 cultivation::MeridianSystem 共享）/ `schema::yidao` / `npc::healer_ai` 子模块（big-brain 节点）
  - agent: `tiandao::yidao_runtime`（5 招 narration + 续命业力代价叙事 + 医患结契 + 化虚群体接经业力累积叙事 + 医者 NPC 行为 AI narration）
  - client: 5 动画（接经针灸 / 排异灸火 / 急救按压 / 续命药引 / 群体接经环阵）+ 5 粒子（平和色脉络可视化）+ 5 音效 recipe + 6 HUD 组件（医者档案 / 患者状态 / 接经手术 UI / 业力累积 / 信誉度 / 医患结契列表）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：接经术真元注入走 `qi_physics::healing::meridian_repair` 🆕 / 排异加速走 `qi_physics::healing::contam_purge` 🆕（worldview §四 排异 tick 反向）/ 急救止血走 `qi_physics::healing::emergency_stabilize` 🆕 / 续命术走 `qi_physics::healing::life_extend` 🆕（含业力代价计算公式）/ 群体接经走 `qi_physics::healing::mass_meridian_repair` 🆕（patch P3 全加）/ **禁止 plan 内自己写治疗 / 排异 / 续命 / 业力公式**

---

## §0 设计轴心

- [ ] **支援流派定调（worldview §六:617 + §五:537）**：医道**与 7 战斗流派平行**，但定位**完全不同**：
  - **战斗流派**（涡流 / 毒蛊 / 替尸 / 截脉 / 体修 / 暗器 / 阵法）：独行 / 对抗 / "靠自己活下去"
  - **支援流派**（医道）：依赖关系 / 长期医患 / "靠互相依赖活下去"
  - 物理代价：所有医道招式都需要"目标患者"（自疗算特殊：把自己当患者）—— 没有"对空 cast"的招式
  - PvP 角色：医者**可被招募 / 雇佣 / 结契**，加入战斗团队作为后援
  - **真元几乎无杀伤性**（worldview §六:617）：医道 cast 不能直接攻击敌人，但可以拒绝治疗敌方（信息战 / 政治战）

- [ ] **5 招完整范围（v1）**：
  ```
  ① 接经术       | 恢复 SEVERED 经脉             | 主路径 → plan-meridian-severed-v1
  ② 排异加速     | 中和 contam（比 zhenmai ② 高 ×3）| worldview §六:617 平和色物理化身
  ③ 急救         | HP 出血止血 + Lifecycle 回暖    | worldview §十二 死亡机制接入
  ④ 续命术       | 临死前真元注入逆转死亡         | worldview §十二:1043 续命物理推导
  ⑤ 群体接经     | 化虚专属 N 患者一次接经         | worldview §三:187 化虚 ×5 凡躯重铸
  ```

- [ ] **化虚级群体接经物理推导（worldview §三:187 + §六:617 + §十二）**：医道的"质变"在化虚级专属——
  ```
  正常接经术：1 患者 1 次手术，cast time 30s，治愈 1 条 SEVERED
  化虚级群体接经：
    化虚医者真元封存场（参考化虚阵法师 + 化虚暗器师同源真元浓度场）
    在场内可同时接经 N 个患者
    N = floor(local_qi_density / threshold)
      化虚医者周围 typical density 9.0，threshold 0.5 → N=18
      mastery 100 → threshold 0.2 → N=45
    每个患者：
      独立手术（独立 raycast 患者经脉拓扑）
      代价 -2% qi_max（化虚医者付出，patient 获得）
      累计业力（每患者 +0.1 业力，N=18 → 一次群体接经 +1.8 业力）
    限制：
      不是免费 AOE 治疗——业力代价和 qi_max 减损按 N 累加
      化虚医者群体接经一次 = 永久 -36% qi_max（N=18）+ 1.8 业力
      跟欺天阵被识破反噬 / 体修散功烧池子 / 截脉绝脉断链同列：化虚级**用永久身体代价换战略效果**
  ```
  - 哲学：化虚医道的"济世"**有真实物理代价**——每救一人，永久失去 2% qi_max，跟支援流派定调一致（"靠互相依赖活下去"——治疗他人也是消耗自己）
  - **不是 MMO 式 AOE 治疗**：每个患者独立手术，业力 + qi_max 代价按 N 累加；患者经脉拓扑差异决定接经成功率（不是 100% 全治愈）

- [ ] **续命术反 MMO 红线（worldview §十二:1048）**：
  - 续命**不是无限复活**，物理代价：
    - **业力累积**：每次续命 +5 业力（业力 ≥10 触发"业障劫"，渡劫期间被天道盯死，对应 worldview §三:368 越级原则物理化身）
    - **qi_max 上限永久减少**：每次续命 -10% qi_max（不可恢复，跟体修散功 -50% qi_max 同源）
    - **境界倒退风险**：50% 概率 → 境界 -1（化虚倒退到通灵，通灵倒退到固元，etc.）
    - **续命窗口**：仅在 Lifecycle 进入 "Dying" 状态后 30s 内可施展（worldview §十二:1043 续命窗口）
  - 续命术不能自我施展（医者用续命术救自己 = 死循环）—— **必须有第二个医者或医者 NPC**

- [ ] **通用 cast 触发模型（接 plan-hotbar-modify-v1 ✅ + `combat::Casting` 标准路径）**：5 招全部走"InspectScreen (I) 拖到 1-9 SkillBar 槽 → 战斗时按数字键 cast"统一路径，**不走右键弹窗菜单**。
  - **目标锁定**：① ② ③ ④ 单目标——玩家先 look 到 5 格（① ②）/ 1 格（③ ④）内目标实体（HUD 高亮患者轮廓 + 距离指示）→ 按数字键 cast；server 在 cast 完成瞬间重新检测距离阈值（**不是 cast 开始时锁定**），中途患者跑出 → 中断走标准路径
  - **群体目标**（⑤ 群体接经）：cast 开始瞬间扫 5 格内全部 SEVERED 患者作为候选 N（cast 时显示 N 计数 HUD），cast 完成瞬间重新结算谁还在 5 格内（跑出去的患者不计入）
  - **触发**：按数字键 → server 创建 `Casting { duration_ticks, start_position }` component → cast bar 显示进度 + 患者 HP / 真元 / SEVERED 经脉图同步 HUD 显示
  - **中断**：① ② ④ ⑤ 不可中断（业力 / 续命 / 群体接经类）—— 但移动超阈值仍中断（医者动了不能继续手术），受击不中断；③ 急救可被中断（5s 短 cast，对应 worldview "急救"语义）
  - **中断后果**：业力 / qi_max 代价不扣（cast 未完成 = 没真正施展）；冷却走中断短冷却
  - 各招"操作"段只写**差异**（cast time / 距离阈值 / 是否可中断 / HUD 反馈细节），通用部分不重复

- [ ] **熟练度生长二维划分（v2 通用机制）**：
  - **境界**：决定治疗上限（接经成功率 / 续命业力减免 / 群体接经 N 上限）
  - **熟练度 mastery (0-100)**：决定接经速度（cast time）/ 排异效率 / 业力减免（mastery 100 续命业力 -50%）/ 群体接经 N 倍率
  - 5 招各自有 `mastery: u8` 字段，cast 一次 +0.5（mastery <50）/ +0.2（50-80）/ +0.05（80-100）
  - 数值表见 §1 各招规格

- [ ] **支援流派身份认知（PvP 招募 / 雇佣 / 结契）**：
  - **招募**：玩家 NPC 主动加入战斗团队（短期，1 场战斗）
  - **雇佣**：玩家 NPC 长期跟随（24h - 1 周，按骨币 / 信誉度结算）
  - **结契**：医患长期关系（永久，互相 +信誉度 + 治疗折扣）
  - 实装路径：plan-social-v1 Reputation 系统扩展

- [ ] **专属物理边界 = 患者目标 + 真元守恒（无对空 cast）**：跟其他流派最大区别：
  - **医道**：所有 cast 必须有 patient target（自疗算自己）；真元从医者付出 → 患者获得（守恒律）
  - **战斗流派**：可以对空 cast / 对环境 cast / 对自己 cast
  - 实装：cast time 期间 patient 必须在 5 格内，离开 → cast 失败
  - 化虚群体接经：N 患者必须在 5 格内（化虚级真元封存场范围）

---

## §1 五招完整规格

### ①「接经术」

**功能**：恢复 SEVERED 经脉（plan-meridian-severed-v1 主路径）

**境界要求**：无 gate（醒灵能学但效率低）

**真元消耗**：50-80% qi_max（医者付出，patient 获得 → ledger 守恒）

**冷却**：cast time 60s → 20s（mastery 0→100）

**效果**：
- 修复 patient 1 条 SEVERED 经脉（接经成功率随境界 + mastery）
- 接经成功率：
  - 醒灵 + mastery 0：30%
  - 化虚 + mastery 100：99%（不是 100%——有 1% 物理失败率，对应 worldview §四 排异 tick）
- 失败：medic 真元浪费 + patient SEVERED 不变 + 双方 +业力 1（接经手术伤害积累）
- 成功：medic 真元转入 patient + patient SEVERED 恢复 + medic 平和色 PracticeLog +50 + medic 信誉度 +1

**操作**（差异）：look 锁定 5 格内患者（HUD 高亮）→ 按数字键 cast，duration 60s（不可中断；移动超阈值中断）；cast 完成瞬间重检 5 格 + SEVERED 经脉图，目标走开 → 中断业力/qi_max 代价不扣

**mastery 生长**：
- cast time 60s → 20s
- 接经成功率公式 +20%
- 失败业力代价 -50%（mastery 100：双方业力 +0.5）

**经脉依赖**（接 plan-meridian-severed-v1）：心经（医者真元广播） + LU 手太阴肺经（针灸主经），SEVERED 任一 → ① 失效

**测试饱和**：接经术 30 单测（成功率 / 失败处理 / 守恒律 / 业力累积 / cast 中断 / mastery 增长）

---

### ②「排异加速」

**功能**：中和 patient contam（比 zhenmai ② 局部中和高 ×3）

**境界要求**：无 gate

**真元消耗**：30-50% qi_max（医者付出 → patient contam 减少）

**冷却**：cast time 30s → 10s（mastery 0→100）

**效果**：
- 减少 patient `Contamination.entries` 总量 30-60%（境界 + mastery 决定）
- 平和色加成：medic `qi_color = PEACE` → 排异效率 ×3（worldview §六:617 物理化身）
- 应用场景：
  - 救脏真元中毒患者（dugu 流派受害者）
  - 救污染真元过载患者（涡流紊流场受害者）
  - 救自身（自疗）

**操作**（差异）：look 锁定 5 格内患者（HUD 高亮 + contam 进度条）→ 按数字键 cast，duration 30s（不可中断；移动超阈值中断）；cast 完成瞬间重检 5 格

**mastery 生长**：
- cast time 30s → 10s
- 减少 contam 倍率 30% → 60%
- 平和色加成持续期 0s → 30s（mastery 100，cast 后 patient 自然排异速度 ×2 持续 30s）

**经脉依赖**：LU 手太阴肺经 + LI 手阳明大肠经（排异主经），SEVERED 任一 → ② 失效

**测试饱和**：排异加速 25 单测（contam 减少 / 平和色加成 / 持续期 / mastery 增长 / dugu 患者特殊路径）

---

### ③「急救」

**功能**：HP 出血止血 + Lifecycle 回暖（worldview §十二）

**境界要求**：无 gate

**真元消耗**：10-30% qi_max（轻量级，可连续 cast）

**冷却**：cast time 5s → 1.5s（mastery 0→100）

**效果**：
- 立即止血 patient（HP 出血 buff 清除）
- 恢复 HP 30% max（仅止血部分，不是治愈伤口）
- Lifecycle "Dying" → "Wounded"（救濒死患者，必须在 Dying 60s 内 cast）
- 不消除 Wounds（伤口仍存在，需后续接经术 + 排异加速治愈）

**操作**（差异）：look 锁定 1 格内患者（HUD 高亮 + HP / Lifecycle 状态）→ 按数字键 cast，duration 5s（**可中断**——移动 / 受击均中断，对应"急救"语义）；cast 完成瞬间重检 1 格

**mastery 生长**：
- cast time 5s → 1.5s
- HP 恢复 30% → 50% max
- Dying 时间窗 60s → 90s（mastery 100，更晚也能救）
- 中断容忍：mastery 100 受 5+ 真元损伤仍 cast

**经脉依赖**：LI 手阳明大肠经（急救止血），SEVERED → ③ 失效

**测试饱和**：急救 25 单测（HP 恢复 / Lifecycle 状态切换 / Dying 窗口 / 中断 / mastery 增长 / 与 alchemy 急救丹联动）

---

### ④「续命术」

**功能**：临死前真元注入逆转死亡（worldview §十二:1043-1048）

**境界要求**：通灵+ 威力门坎（醒灵 → 凝脉 cast 不出来，因为续命需要高密度真元控制）

**真元消耗**：100-150% qi_max（超量，可分次充能）+ 1 颗续命丹（plan-alchemy-v1 接入）

**冷却**：3600s → 1800s（mastery 0→100，含业力积累冷却）

**效果**：
- patient 从 "Dying" → "Alive" Lifecycle 切换
- patient HP 恢复 50% max
- patient SEVERED 不恢复（接经术继续治疗）
- **代价（worldview §十二:1048）**：
  - medic +5 业力 ≥10 触发"业障劫"
  - medic -10% qi_max（永久不可恢复）
  - patient 50% 概率境界 -1（续命副作用）
  - patient -10% qi_max（永久不可恢复）
- **续命窗口**：Lifecycle "Dying" 后 30s 内（worldview §十二:1043）

**操作**（差异）：look 锁定 1 格内进入 Dying 状态的患者（30s 续命窗口内）→ 按数字键 cast，duration 30s（不可中断；移动超阈值中断）；cast 完成瞬间重检 1 格 + Dying 状态有效，目标已 Terminated → 中断业力 / qi_max 代价不扣

**mastery 生长**：
- cast time 30s → 12s
- medic 业力代价 +5 → +2.5
- patient 境界 -1 概率 50% → 25%
- 续命窗口 30s → 60s（mastery 100，更晚也能救）

**经脉依赖**：心经 + LU + LI + KI（高级真元控制），SEVERED 任一 → ④ 失效

**触发天道注视**（接 plan-tribulation-v1）：每次续命 +tribulation_weight × 1.0（业力累积）

**反 MMO 红线**：见 §0 第 4 条 — 续命**不是无限复活**，物理代价（业力 + qi_max + 境界倒退风险）

**测试饱和**：续命术 35 单测（业力累积 / qi_max 减损 / 境界倒退概率 / 续命窗口 / 续命丹联动 / 业障劫触发 / mastery 增长）

---

### ⑤「群体接经」(化虚专属 / v1 新增)

**功能**：化虚医者一次治愈 N 个 SEVERED（worldview §三:187 化虚 ×5 凡躯重铸 + §六:617 平和色物理推导）

**境界要求**：**化虚 gate**（仅化虚境玩家解锁）—— 跟其他流派化虚专属同列

**真元消耗**：100% qi_max + N × 续命丹 1 颗（每患者一颗）

**冷却**：3600s → 1800s（mastery 0→100，含业力累积冷却）

**效果**：
- N 患者一次接经，N = floor(local_qi_density / threshold)
  - 化虚医者周围 typical density 9.0，threshold 0.5 → N=18
  - mastery 100 → threshold 0.2 → N=45
- 每患者独立手术（独立 raycast 经脉拓扑）+ 独立成功率（与 ① 接经术相同）
- 代价：medic -2% qi_max / 每患者，N=18 → 一次群体接经 -36% qi_max
- 业力：medic +0.1 业力 / 每患者，N=18 → +1.8 业力

**操作**（差异）：按数字键 cast 瞬间扫 5 格内全部 SEVERED 患者作为候选 N（HUD 显示 N 计数 + 每患者高亮）→ duration 60s（不可中断；移动超阈值中断）；cast 完成瞬间重新结算 5 格内有效患者数 N'（跑出去的不计入），按 N' 累加业力 / qi_max 代价（不是 cast 开始时的 N）

**mastery 生长**：
- cast time 60s → 20s
- threshold 0.5 → 0.2 (N 倍数 ×2.5)
- 业力代价 +0.1 → +0.05 / 每患者
- qi_max 减损 -2% → -1% / 每患者

**经脉依赖**：督脉 `Du` + 心经 `Heart` + 肺经 `Lung` + 大肠经 `LargeIntestine` + 肾经 `Kidney` 全套（化虚级最严格依赖；督脉同 anqi-v2 ⑤ / zhenfa-v2 ⑤ 化虚级共享——worldview §201 化虚 20 经脉全开无第 21 脉，§597 任督=统御），SEVERED 任一 → ⑤ 失效（督脉 SEVERED = 跨周目永久残废）

**触发天道注视**（接 plan-tribulation-v1）：群体接经 N≥10 触发天道注视累积，跟 zhenmai-v2 / baomai-v3 / anqi-v2 / zhenfa-v2 化虚级同列

**反 MMO 红线**：见 §0 第 3 条 — **不是 MMO 式 AOE 治疗**，是真实 N 个独立接经手术，业力 + qi_max 代价按 N 累加；患者经脉拓扑差异决定接经成功率

**测试饱和**：群体接经 32 单测（N 上限 / 独立手术 raycast / 业力累积 / qi_max 减损 / 督脉 `Du` + 心 / 肺 / 大肠 / 肾全套依赖 / 天道注视触发 / 患者经脉拓扑差异 / mastery 增长）

---

## §2 平和色养成

接 `plan-multi-style-v1` ✅ PracticeLog vector：

**平和色 dimension**：`PracticeLog.peace_dim` (0-100)，由"治疗"action 累积：

| Action | peace_dim 增益 |
|---|---|
| 接经术成功 | +50 |
| 排异加速成功 | +30 |
| 急救成功 | +10 |
| 续命术成功 | +200 |
| 群体接经成功 | +100 × N |

平和色形成：累计 ~10h 治疗时间出"主色调"（worldview §六:624 染色规则）。

**平和色加成**（worldview §六:617）：
- 接经术 cast time -20%
- 排异加速效率 ×3（独立加成，不与 mastery 叠乘）
- 续命业力代价 -10%
- 群体接经 N 上限 +5

**专精/双修/杂色**（worldview §六:627）：
- 1 主色（平和） → 医道效果满
- 1 主 + 1 副（平和 + 温润 / 缜密 等）→ 70% 折扣
- 三种以上 → 杂色，所有医道专精失效

**v2 实装**：P0 阶段交付（PracticeLog 已实装）。

---

## §3 医者 NPC 行为 AI

接 `plan-npc-ai-v1` ✅ big-brain Utility AI：

**HealerNpc** 行为节点（big-brain Scorer → Action 模式）：

| Scorer | 触发条件 | Action |
|---|---|---|
| `WoundedPatientScore` | 5 格内有 patient HP < 50% | 主动靠近 + 急救 |
| `SeveredPatientScore` | 5 格内有 patient SEVERED 且玩家请求 | 接经术 |
| `ContamPatientScore` | 5 格内有 patient contam ≥50 | 排异加速 |
| `DyingPatientScore` | 5 格内有 patient Lifecycle Dying | 续命术（如果有续命丹 + medic 通灵+） |
| `CombatThreatScore` | 5 格内有敌对玩家 + 战斗中 | 撤退（医道无攻击性 worldview §六:617） |
| `IdleScore` | 无患者 | 灵龛守候 / 采药 / 制丹 |

**HealerNpc 状态推送**：`HealerNpcAiState` server-data → client 显示医者当前活动 + 信誉度 + 接诊队列。

**v1 实装**：P2 阶段交付（big-brain 节点 + Scorer + Action）。

---

## §4 信誉度 / 长期医患关系

接 `plan-social-v1` ✅ Reputation 系统：

**医患关系状态机**（`MedicalContract` component）：

| 状态 | 触发 | 效果 |
|---|---|---|
| Stranger | 默认 | 无加成 |
| Patient | 接受过 1+ 治疗 | medic 信誉度 +1 / 治疗 |
| Long-term Patient | 累计 5+ 治疗 + 30 天关系 | medic 信誉度 +3 / 治疗，patient 治疗折扣 -10% |
| Bonded（结契） | 双方主动签订（worldview §十一:1416 灵龛见证）| medic + patient 永久互相 +信誉度 +5 / 治疗，patient 治疗折扣 -30%，medic 平和色养成 ×1.5 |

**结契仪式**：双方在灵龛旁同时执行 `bong:yidao/bond_contract` request → 灵龛见证 → MedicalContract 写入 server-data。

**违约惩罚**：结契后医者拒绝治疗 → patient 信誉度 -10 + medic 信誉度 -20（worldview §十一医患信誉物理化身）。

**v1 实装**：P3 阶段交付（接 plan-social-v1 + plan-narrative-political-v1 联动）。

---

## §5 客户端动画 / VFX / SFX

| 招式 | 动画 | 粒子 | 音效 |
|---|---|---|---|
| ① 接经术 | 双手持针对患者经脉点（30 个穴位顺序） | 平和色经脉脉络可视化 + 针入皮 | 针刺嗡 + 经脉通流嗡（持续）+ 完成叮 |
| ② 排异加速 | 双手对患者掌心灸火（10 个排异点） | 平和色烟雾 + contam 红雾被中和散去 | 灸火嘶嘶 + 排异叹息 |
| ③ 急救 | 双手按压患者胸口（CPR-style） | 平和色光晕环绕患者胸腔 | 按压嘭 + 复苏吸气 |
| ④ 续命术 | 持续命丹喂患者 + 单手对天接引 | 业力黑雾从天降 + 平和色光柱接引 + 患者真元颜色暂时翻转 | 续命术咏唱（持续 30s）+ 业力雷鸣 |
| ⑤ 群体接经 | 中心持化虚平和色法器 + N 患者环阵围 | N 个独立平和色脉络可视化 + 业力黑雾 N 波 | 化虚共振嗡（持续 60s）+ N × 完成叮（错峰）+ 业力雷鸣 N 次 |

HUD 组件（plan-HUD-v1 接入）：

- **医者档案**：医者 NPC inspect 显示信誉度 + 平和色 mastery + 接诊队列 + 业力等级
- **患者状态**：medic cast 时显示 patient HP / 真元 / SEVERED 经脉图 / contam / Lifecycle 状态
- **接经手术 UI**：cast time 显示当前正在治疗的经脉 + 成功率 + 业力预测代价
- **业力累积**：HUD 圆环显示 medic 当前业力 + 距离业障劫的进度（≥10 触发）
- **信誉度**：HUD 显示 medic 信誉度（影响治疗折扣 + 接诊队列优先级）
- **医患结契列表**：HUD 列表显示当前结契患者 + 信誉度 + 关系强度
- **化虚群体接经预览**：化虚医者 cast 时显示候选 N 患者列表 + 业力 / qi_max 代价预测

**v1 实装**：P4 阶段交付。

---

## §6 阶段交付物（P0 → P5）

### P0 — 接经术 + 排异加速 + 平和色养成 底盘（4-6 周）

- [ ] `combat::yidao` 主模块（治疗系统接入 cultivation::MeridianSystem + combat::Wounds + combat::Contamination）
- [ ] `YidaoSkillId` enum 5 招注册到 SkillRegistry
- [ ] `qi_physics::healing::meridian_repair` + `contam_purge` 算子（patch P3 加）
- [ ] `MeridianHealEvent` + `ContamPurgeEvent` schema
- [ ] ① 接经术 + ② 排异加速实装（含 plan-meridian-severed-v1 接经术主路径）
- [ ] 平和色 PracticeLog vector 接入（peace_dim 0-100）
- [ ] library 锚点登记：`peoples-0007 医者列传`（3-5 位医者人物）+ `cultivation-0002 §针灸论` 补章
- [ ] 测试：接经 30 单测 + 排异 25 单测 + 平和色养成 12 单测

### P1 — 急救 + 续命术 + 业力 / 业障劫（4-5 周）

- [ ] `qi_physics::healing::emergency_stabilize` + `life_extend` 算子（patch P3 加）
- [ ] `EmergencyResuscitateEvent` + `LifeExtensionEvent` + `KarmaAccumulationEvent` schema
- [ ] ③ 急救 + ④ 续命术实装
- [ ] 业力累积系统（KarmaCounter component）+ 业障劫触发（≥10 + 接 plan-tribulation-v1）
- [ ] 续命丹接入（plan-alchemy-v1 PillRegistry）
- [ ] 测试：急救 25 单测 + 续命 35 单测 + 业力 / 业障劫 15 单测

### P2 — 医者 NPC 行为 AI（big-brain）（3 周）

- [ ] `npc::healer_ai` 子模块（big-brain 节点）
- [ ] HealerNpc 6 Scorer + 6 Action 节点（见 §3）
- [ ] `HealerNpcAiState` server-data 推送
- [ ] 医者 NPC spawn 路径（worldview §十一 灵龛守候）
- [ ] 测试：HealerNpc AI 25 单测 + Scorer / Action 各 5 单测 + spawn 5 单测

### P3 — 信誉度 + 医患结契 + 经脉依赖 + 熟练度生长（4-5 周）

- [ ] 接 `plan-social-v1` Reputation 扩展
- [ ] `MedicalContract` component 4 状态机（Stranger / Patient / Long-term / Bonded）
- [ ] 结契仪式（灵龛见证 → server-data 写入）
- [ ] 接 `plan-meridian-severed-v1` 7 流派经脉表追加医道条目（接经术主路径）
- [ ] 5 招 mastery 字段（0-100）+ cast 加 mastery 公式
- [ ] mastery 生长效果（cast time / 成功率 / 业力代价 / N 上限）
- [ ] 违约惩罚（接 plan-narrative-political-v1）
- [ ] 测试：信誉度 12 单测 + 医患结契 18 单测 + 经脉依赖 25 单测 + mastery 30 单测

### P4 — 化虚群体接经 + 客户端 5 动画 / 5 粒子 / 6 HUD（4 周）

- [ ] `qi_physics::healing::mass_meridian_repair` 算子（patch P3 加）
- [ ] ⑤ 群体接经招式实装（化虚 gate + N 患者 + 业力 / qi_max 代价）
- [ ] 5 动画 + 5 粒子 + 5 音效 recipe
- [ ] 6 HUD 组件（医者档案 / 患者状态 / 接经手术 / 业力 / 信誉度 / 医患结契列表 / 群体接经预览）
- [ ] 测试：群体接经 32 单测 + 客户端视觉回归 + HUD 集成测试

### P5 — v1 收口（饱和测试 + agent narration + e2e 联调）（2-3 周）

- [ ] agent `tiandao::yidao_runtime`（5 招 narration + 续命业力代价 + 医患结契 + 化虚群体接经业力累积叙事 + HealerNpc AI narration）
- [ ] 化虚医者业力江湖传闻 + 业障劫叙事
- [ ] e2e 联调：client → server cast → 治疗事件 → patient 状态变化 → client 渲染（每招独立 e2e）
- [ ] 饱和测试 audit：5 招每招 ≥20 单测，总单测 ≥150
- [ ] 与 plan-style-balance-v1 对接：医道 ρ=0.05 + 治疗效率矩阵填表
- [ ] Finish Evidence + 迁入 docs/finished_plans/

---

## §7 已知风险 / open 问题

- [ ] **Q1** 化虚群体接经 N 上限：mastery 100 + 高密度区 = 45 患者？还是 30？性能 / 平衡 / 业力代价哪个优先 → P4 拍板
- [ ] **Q2** 续命术境界倒退概率（50%）：是否需要根据 medic mastery 减小 → P1 拍板
- [ ] **Q3** 业障劫（业力 ≥10 触发）：是否给医道流派减免（worldview §十二 续命物理）→ P1 与 plan-tribulation-v1 联动
- [ ] **Q4** 医者 NPC 拒绝治疗（worldview §六:617 真元几乎无杀伤性）：医者怎么"拒绝敌人治疗"——主动 cast 失败 vs 战斗时撤退 vs 被强制治疗（PvP 中绑架医者）→ P2 拍板
- [ ] **Q5** 医患结契仪式：双方需要同时在场吗？仪式中断怎么处理？→ P3 拍板
- [ ] **Q6** 跨周目（plan-multi-life-v1）：业力 / 平和色 / 医患结契是否继承？→ P3 与 multi-life 联动
- [ ] **Q7** 亚流派扩展（毒手医 / 兽医 / 道伥医）：哪个先做 v2？→ 留 plan-yidao-v2 拍板
- [ ] **Q8** 续命丹副作用 vs 续命术副作用叠加：用续命丹后 cast 续命术，副作用是否双倍？→ P1 与 plan-alchemy-v1 联动
- [ ] **Q9** 急救 cast 中断：mastery 0 受 1 真元损伤即中断，太苛刻？→ P1 调参

---

## §8 进度日志

- 2026-05-06：骨架创建。worldview §六:617 已锚定平和色 + 医道流派定义。v1 范围：5 招完整包（接经术 / 排异加速 / 急救 / 续命术 / 化虚群体接经）+ 平和色养成 + 医者 NPC 行为 AI（big-brain）+ 信誉度 / 医患结契 / 续命业力代价 / 化虚级群体接经物理推导。化虚群体接经走 worldview §三:187 + §六:617 平和色物理（**不是 MMO 式 AOE 治疗**，每患者独立手术，业力 + qi_max 代价按 N 累加）。续命术走 worldview §十二:1043-1048 续命物理（**不是无限复活**，业力 + qi_max -% + 境界倒退风险）。支援流派定调，与 7 战斗流派"独行 / 对抗"区分（"靠互相依赖活下去"——医患结契 / 长期关系）。
- **2026-05-09**：升 active（`git mv docs/plans-skeleton/plan-yidao-v1.md → docs/plan-yidao-v1.md`）。触发条件：
  - **plan-qi-physics-patch-v1 ✅ finished**（PR #162，2026-05-08）—— ρ 矩阵 + W 矩阵 + 接经/排异/急救/续命/群体接经 5 算子接入路径就位
  - **plan-meridian-severed-v1 ✅ finished** —— 接经术目标系统就位
  - **plan-skill-v1 ✅** + **plan-craft-v1 ✅** + **plan-multi-style-v1 ✅** + **plan-alchemy-v1 ✅** + **plan-npc-ai-v1 ✅** + **plan-social-v1 ✅** —— 全部 6 个前置都已 finished
  - 用户 2026-05-09 拍板**音效/特效/HUD 区分硬约束**：5 招 cast 必须各自携带差异化 animation + particle + SFX + HUD 反馈（接经穴位针 vs 排异灸火 vs CPR 按压 vs 续命咏唱 vs 化虚群体环阵），§5 已覆盖；P0/P1/P4 验收必须包含视觉/听觉差异化回归测试，单方向 stub 实装不收
  - 下一步：进 P0（接经术 + 排异加速 + 平和色养成 底盘 4-6 周），同步与 yidao-v1 §7 九开放问题逐一收口

## Finish Evidence

### 落地清单

- **Server / qi_physics**：`server/src/qi_physics/healing.rs` 落地接经、排异、急救、续命、群体接经 5 个治疗算子；`server/src/qi_physics/ledger.rs` 增加 `QiTransferReason::Healing`。
- **Server / combat**：`server/src/combat/yidao.rs` 注册 `yidao.meridian_repair`、`yidao.contam_purge`、`yidao.emergency_resuscitate`、`yidao.life_extension`、`yidao.mass_meridian_repair` 5 招，接入 SEVERED 经脉修复、污染排异、NearDeath 急救、续命永久代价、化虚群体接经、`HealerProfile`、`HealingMastery`、`KarmaCounter`、医患 treatment contract 与医者 NPC 决策；review 修复确认失败接经不声明 qi transfer、续命拒绝自目标、mastery 按 cast 增长而信誉 / 契约按成功患者记录，并把医道治疗效果延后到 `Casting` 完成事件后结算，中断 cast 不再产生治疗结果。
- **Server / IPC**：`server/src/schema/yidao.rs`、`server/src/schema/channels.rs`、`server/src/network/redis_bridge.rs` 新增 `YidaoEventV1` 与 `bong:yidao/event`；`server/src/network/yidao_state_emit.rs` 下发 `HealerNpcAiStateV1` / `YidaoHudStateV1`，并在 review 修复中补齐迟到客户端稳定快照与 HUD 空投影清理路径。
- **Server / audio**：`server/assets/audio/recipes/yidao_*.json` 新增 5 个医道音效 recipe，并由 `server/src/audio/mod.rs` 默认加载测试固定。
- **Agent / schema**：`agent/packages/schema/src/yidao.ts`、`server-data.ts`、`channels.ts`、`schema-registry.ts` 增加 `YidaoEventV1`、`YidaoSkillIdV1`、`MedicalContractStateV1`、`HealerNpcAiStateV1`、`YidaoHudStateV1`，并生成对应 JSON schema。
- **Agent / tiandao**：`agent/packages/tiandao/src/yidao-runtime.ts` 与 `main.ts` 接入医道事件订阅和 5 招叙事渲染。
- **Client / HUD 与视觉**：`client/src/main/java/com/bong/client/yidao/*`、`YidaoHudPlanner.java`、`BongHudOrchestrator.java`、`ServerDataRouter.java` 接入医道 HUD / 医者 NPC AI 状态；`YidaoPeacePulsePlayer.java` 与 `VfxBootstrap.java` 注册 5 个平和色 VFX；review 修复确认空 HUD 投影会清除旧面板、负信誉不会被客户端钳成 0、异常粒子原点/时长不会污染渲染。

### 关键 commit

- `7981cd4a4` · 2026-05-09 · `feat(server): 落地医道五招与下发契约`
- `0ef8e3529` · 2026-05-09 · `feat(agent): 接入医道 schema 与叙事运行时`
- `124f01443` · 2026-05-09 · `feat(client): 渲染医道 HUD 与平和色视听反馈`
- `3a811d57e` · 2026-05-09 · `fix(yidao): 收紧音效契约与 HUD 下发测试`
- `d71008247` · 2026-05-09 · `fix(yidao): 修复无效治疗事件与续命窗口边界`
- `b748e9ef1` · 2026-05-09 · `fix(yidao): 收敛医道状态同步 review 问题`
- `aa5b1e1d6` · 2026-05-09 · `fix(yidao): 收紧医道治疗结算边界`

### 测试结果

- `cd server && cargo fmt --check` ✅
- `cd server && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings` ✅
- `cd server && CARGO_BUILD_JOBS=1 cargo test` ✅ `3251 passed; 0 failed`
- `cd server && CARGO_BUILD_JOBS=1 cargo test yidao` ✅ `22 passed; 0 failed`
- `cd server && cargo test healing` ✅ `7 passed; 0 failed`
- `cd agent && npm run generate -w @bong/schema` ✅，生成物无未提交漂移
- `cd agent && npm run build` ✅
- `cd agent && npm test -w @bong/schema` ✅ `13 files / 331 tests passed`
- `cd agent && npm test -w @bong/tiandao` ✅ `43 files / 308 tests passed`
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build` ✅ `BUILD SUCCESSFUL`
- `git diff --check` ✅

### 跨仓库核验

- **Server**：`YidaoEventV1`、`HealerNpcAiStateV1`、`YidaoHudStateV1`、`CH_YIDAO_EVENT`、`RedisOutbound::YidaoEvent`、`ServerDataPayloadV1::HealerNpcAiState`、`ServerDataPayloadV1::YidaoHudState`。
- **Agent**：`CHANNELS.YIDAO_EVENT = "bong:yidao/event"`、`YidaoNarrationRuntime`、`validateYidaoEventV1Contract`、`validateHealerNpcAiStateV1Contract`、`validateYidaoHudStateV1Contract`。
- **Client**：`YidaoServerDataHandler`、`YidaoHudStateStore`、`YidaoNpcAiStateStore`、`YidaoHudPlanner`、`YidaoPeacePulsePlayer`、`healer_npc_ai_state` / `yidao_hud_state` 路由。
- **5 招差异化资源**：`bong:yidao_meridian_repair`、`bong:yidao_contam_purge`、`bong:yidao_emergency_resuscitate`、`bong:yidao_life_extension`、`bong:yidao_mass_meridian_repair`；对应 `yidao_*.json` audio recipe 已纳入默认 registry。

### 遗留 / 后续

- `docs/library/` 属于 library-curator 责任域，plan 流水线未主动回写 `peoples-0007 医者列传` 或 `cultivation-0002 §针灸论`。
- 业力深链、业障劫叙事、跨周目继承与亚流派扩展留给 `plan-tribulation-v1`、`plan-multi-life-v1`、`plan-yidao-v2` 继续深化；本 v1 已提供 `KarmaCounter`、`YidaoEventV1` 与医道 contract state 的运行时接入面。
