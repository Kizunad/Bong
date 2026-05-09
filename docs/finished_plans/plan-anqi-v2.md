# Bong · plan-anqi-v2

器修·暗器流功法**五招完整包**：动画 / 特效 / 音效 / 伤害 / 真元消耗 / 载体磨损 / 多容器 / 客户端 UI 全流程。承接 `plan-anqi-v1` ✅ finished（PR + 2026-05-04 commit；P0 单射狙击 + 异变兽骨单档载体 + hand-slot 已实装）—— v2 引入**6 档载体全开**（残骨 → 异变兽骨 → 灵木 → 凝实色染色骨 → 封灵匣骨 → 上古残骨）+ **多发齐射**（散花扇形 + 凝魂注射多目标）+ **凝魂注射 / 破甲注射 / 诱饵战术** 三种新功法 + **多容器**（箭袋 / 裤袋 / 封灵匣，入囊 ↔ 出囊磨损税）+ **化虚专属诱饵分形**（worldview §P 真元浓度场扰动，一发兽骨在化虚境真元场内分裂为 N 个 echo 载体，全部独立飞行；**不是分身无敌**，是真实 N 弹道、可被范围伤害挡住任意一支）+ **5 招完整规格**（单射狙击 / 多发齐射 / 凝魂注射 / 破甲注射 / 诱饵分形），无境界 gate 只有威力门坎。

**世界观锚点**：`worldview.md §五.2 器修/暗器流`（line 405-413：载体材质分级 50 格保留 80%）· `§五:462 primary axis`（载体封存比例 + 命中距离）· `§四:332-340 距离衰减`（贴脸 100% / 10 格 40% / 50 格归零；异变兽骨 50 格 80%）· `§四:354 过载撕裂`（凝魂注射 5/s safe limit 反向推导）· `§四:360-391 越级原则`（>30% qi_max 注射触发战后虚脱 = v1 Q44 留待问题）· `§六:480 毒性真元/凝实色泛型化`（凝魂注射要求载体染色匹配）· `§六:542 凝实色 × 器修原生匹配`· `§十一:1416 封灵匣`（异兽骨骼/灵木编制；不入箱使用不扣次数）· `§P 真元浓度场`（化虚级诱饵分形物理依据）· `§三:187 化虚 ×5 凡躯重铸`（化虚级招式物理推导前提）· `§K narration 沉默`

**library 锚点**：`peoples-0005 异变图谱·残卷`（兽爪 10 骨币 / 兽核 80 骨币 / 缝合兽来源 → 6 档载体素材） · `peoples-0006 战斗流派源流` 攻击三·器修/暗器流原文 · `ecology-0004 灵物磨损笔记`（"暗器载体每入囊出囊扣一成真元 / 故器修持骨于手不入囊" → v2 磨损税正典实装） · `ecology/绝地草木拾遗`（云顶兰 / 悬根薇 / 渊泥红玉 → 高阶载体来源）

**前置依赖**：

- `plan-anqi-v1` ✅ → 单射狙击 + 异变兽骨 + hand-slot + 半衰期 + 飞行衰减实装就位（v2 在此基础上扩档）
- `plan-qi-physics-v1` P1 ship → 真元浓度场扰动 / 距离衰减 / 注射比例走 `qi_physics::projectile::*`（散射、追形不允许 plan 内自己写公式）
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 ρ/W/β 矩阵（暗器 ρ=0.30 / W vs 4 攻 [0.4, 0.5, 0.3, 0.7]）+ overload-tear 算子（凝魂注射 >30% qi_max 触发）+ density-echo 算子（化虚诱饵分形 echo 数公式）
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ + `plan-multi-style-v1` ✅
- `plan-craft-v1` 🟡 → AnqiCarrier 类目（6 档载体 + 3 容器配方）—— v2 P0 必须先用此底盘
- `plan-meridian-severed-v1` 🆕 active → 暗器流派依赖经脉清单（手三阴之一 / 心包经 Pericardium / 脾经 Spleen / 大肠经 LargeIntestine / 督脉 Du）+ SEVERED 招式失效路径
- `plan-color-v1`：**已被替代**（plan-anqi-v1 vN+1 留待 Q41 已注），染色系统归到 `cultivation::QiColor` 自身扩展 —— v2 凝魂注射的 "凝实色匹配" 直接读 `Cultivation.qi_color`
- `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅
- `plan-cultivation-canonical-align-v1` ✅ → Realm + 经脉拓扑选择
- `plan-tsy-loot-v1` ✅ → 6 档载体素材掉率（v1 Q42 留待问题，v2 落地）
- `plan-tsy-hostile-v1` ✅ → 异变缝合兽掉异变兽骨概率调参（v1 默认 30%，v2 调）

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 招的 W/ρ 数值进矩阵（暗器 ρ=0.30 / 专克体修 W=0.7 / 失效 vs 截脉 W=0.0 因接触面音论反震集中）
- `plan-tribulation-v1` ⏳ → 化虚级诱饵分形 echo 数 ≥ 30 触发天道注视
- `plan-narrative-political-v1` ✅ active → 化虚暗器师诱饵分形战场叙事（"一支骨刺裂出三十支" 江湖传闻）
- `plan-yidao-v1` 🆕 placeholder → 凝魂注射 / 破甲注射做为暗器派 hard counter 医道（医者补脉时段被截杀路径）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { qi_current, qi_max, realm, contamination, qi_color }` / `cultivation::MeridianSystem` / `qi_physics::projectile::*`（弹道、衰减、追踪、密度场） / `qi_physics::ledger::QiTransfer`（真元注射守恒律） / `combat::Wounds` / `combat::Contamination` / `combat::carrier::CarrierImprint`（v1 已实装，v2 扩 6 档） / `combat::projectile::AnqiProjectileFlight`（v1 已实装，v2 扩 echo / multi-shot 字段） / `combat::needle::*`（v1 已实装，v2 借鉴速度/距离常量） / `inventory`（载体素材 + 容器槽位） / `craft::CraftRegistry`（6 档载体配方 + 3 容器配方） / `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm`
- **出料**：5 招 `AnqiSkillId` enum 注册到 SkillRegistry / `MultiShotEvent` 🆕（齐射多发独立 raycast）/ `QiInjectionEvent` 🆕（凝魂 / 破甲注射伤害事件）/ `DecoyDeployEvent` 🆕（诱饵布设）/ `EchoFractalEvent` 🆕（化虚诱饵分形 N 弹道）/ `CarrierAbrasionEvent` 🆕（入囊 ↔ 出囊磨损税扣真元）/ `ContainerSwapEvent` 🆕（多容器槽位切换）/ `OverloadTearTriggered` 🆕（>30% qi_max 注射触发战后虚脱，v1 Q44 留待）
- **共享类型**：`StyleProjectile` trait（qi_physics::traits）/ `CarrierGrade` enum 扩 6 档（v1 单档 → v2 6 档）/ `ContainerSlot` component（箭袋 / 裤袋 / 封灵匣 / hand-slot 4 选 1，与 v1 hand-slot 兼容）/ `CarrierImprint`（v1 已实装，v2 扩 `injection_kind: Option<InjectionKind>` 字段）
- **跨仓库契约**：
  - server: `combat::anqi_v2::*` 主实装（v1 单射迁入 + v2 新 4 招）/ `schema::anqi_v2`
  - agent: `tiandao::anqi_v2_runtime`（5 招 narration + 6 档载体语义 + 诱饵分形战场叙事 + 凝魂注射"凝实色匹配"叙事 + 入囊扣真元提示）
  - client: 5 动画 + 4 粒子 + 5 音效 recipe + 6 HUD 组件（载体格栅 / 容器栏 / 凝魂条 / 诱饵 marker / echo 计数 / 磨损 tooltip）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：弹道走 `qi_physics::projectile::flight_decay`（v1 已用）/ 散射多发走 `qi_physics::projectile::cone_dispersion` 🆕（patch P3 加）/ 凝魂注射走 `qi_physics::projectile::high_density_inject` 🆕（密度封存）/ 破甲注射走 `qi_physics::projectile::armor_penetrate` 🆕（无视防御 75-90%）/ 化虚分形 echo 走 `qi_physics::field::density_echo` 🆕（patch P3 加新算子，echo 数 = floor(local_qi_density / threshold)）/ 磨损税走 `qi_physics::container::abrasion_loss` 🆕（patch P3 加，每入/出囊 10% 真元损失）/ **禁止 plan 内自己写衰减 / 散射 / 注射 / echo 公式**

---

## §0 设计轴心

- [ ] **物资派定调（worldview §五.2 + ecology-0004）**：暗器流派和体修截脉血肉派、毒蛊脏真元派区分，**用钱包代价换战场远射**。每招物理代价：
  - ① 单射狙击 → 一根载体烧 20-30% qi_max（v1 已实装）
  - ② 多发齐射 → 烧 5 根载体 / 1 次释放（5 倍材料代价）
  - ③ 凝魂注射 → 单根高密度封存（>30% qi_max → 触发战后虚脱）
  - ④ 破甲注射 → 单根超功率封存（载体即时碎裂概率 50%）
  - ⑤ 诱饵分形（化虚）→ 一根载体 + 化虚级真元浓度场 → 30+ echo 载体（**所有 echo 都消耗本体一根载体的真元**，物理上一根载体被分形场打散为 N 个真元投影，全部命中后载体一同碎裂）

- [ ] **载体 6 档分级（worldview §五:405-413 + library peoples-0005）**：
  ```
  档 1 残骨        | 普通动物骨   | 5 骨币 / 根   | 30 格保留 50% | 杂色 ρ=0.6
  档 2 异变兽骨    | v1 已实装    | 80 骨币 / 根  | 50 格保留 80% | 半凝色 ρ=0.4
  档 3 灵木编制    | 飞针/暗箭    | 150 骨币 / 把 | 30 格保留 70% | 杂色 ρ=0.5（多发齐射专用）
  档 4 凝实色染色骨| 染色养成     | 400 骨币 / 根 | 50 格保留 95% | 凝实色 ρ=0.2（凝魂注射专用）
  档 5 封灵匣骨    | 上古残骨改   | 1200 骨币 / 根| 80 格保留 90% | 凝实色 ρ=0.15（破甲注射专用）
  档 6 上古残骨    | 末法残土遗存 | 5000 骨币 / 根| 150 格保留 90%| 凝实色 ρ=0.10（化虚诱饵分形专用，echo 数依赖载体纯度）
  ```
  - 每档载体的 `(decay_curve, dispersion_factor, injection_compat, density_threshold)` 走 qi_physics 算子参数化，**plan 内只声明档位标识**

- [ ] **化虚级诱饵分形（worldview §P + §三:187 + §四:354 反向推导）**：暗器的"质变"在化虚级专属——主动注入超量真元（>50% qi_max），载体在化虚级真元浓度场中**真实分形**：
  ```
  正常载体单弹道 → 化虚境真元场扰动 → 分形 N 个真元投影 echo
  echo 数 = floor(local_qi_density / threshold)
    threshold = 0.3 (worldview §P 浓度场扰动阈值)
    化虚级 local_qi_density 典型 9.0 → 30 个 echo
    熟练度 mastery=100 阈值降至 0.1 → 90 个 echo（plan-style-balance v1 调）

  每个 echo:
    独立 raycast（可被范围伤害挡住任意一支）
    独立伤害 = base / N（总伤害和单弹道一致，不是叠加，**不是无敌分身**）
    独立飞行轨迹（轻微随机扰动，模拟真元场不均匀）

  载体本身（root projectile）：
    所有 echo 命中或脱靶后碎裂
    若 ≥1 个 echo 命中目标 → 载体真元注入消耗为 1 根载体（不是 30 根）
    若全部 echo 脱靶 → 载体半衰期归零（v1 半衰 120min 跳过）
  ```
  - 哲学：化虚暗器师**用真元浓度场把一根载体打散为 N 弹道**，跟物资派定调一致——载体仍是一根（钱包不变），但战场覆盖率 ×N
  - **不是免疫无敌**：每个 echo 都是真实碰撞实体，被范围伤害（涡流涡心 / 截脉多点反震 / 体修撼山）挡住后该 echo 失效。30 echo 全失效则诱饵分形浪费

- [ ] **专属物理边界 = 单根载体 + 单点封存（vs 截脉血肉广分布）**：跟其他流派最大区别：
  - **暗器**：单根载体 + 单点真元封存 → 单点高密度释放（凝魂 / 破甲）/ 多点扩散（齐射 / 分形）
  - **截脉**：自身经脉 + 接触面分布 → 反震
  - **毒蛊**：自身脏真元 + 持续渗透 → 长期污染
  - **替尸**：物资载体 + 一次性吸收 → 物理免疫一次冲击（不是真元免疫）
  - **体修**：自身经脉过载 → 凡躯重铸
  - **涡流**：环境真元场扰动 → 紊流
  - **地师阵法**：地形坐标 + 触发 → 区域控制
  - **暗器在 W 矩阵**：vs 体修 W=0.7（贴脸单点入肉）/ vs 器修 W=0.5 / vs 截脉 W=0.0（接触面音论高频反震完全克制）/ vs 毒蛊 W=0.4 / vs 替尸 W=0.6 / vs 阵法 W=0.5

- [ ] **熟练度生长二维划分（v2 通用机制）**：
  - **境界**：决定威力上限（醒灵微弱 / 化虚惊天）
  - **熟练度 mastery (0-100)**：决定**响应速率**（瞄准准星收紧速度 / 弹道精度散射收敛 / 注射封存时间 / echo 数 / 容器切换速度）
  - 5 招各自有 `mastery: u8` 字段（0-100），cast 一次 +0.5（mastery <50）/ +0.2（50-80）/ +0.05（80-100），上限 100
  - 数值表见 §1 各招规格

- [ ] **通用 cast 触发模型（接 plan-hotbar-modify-v1 ✅ + `combat::Casting` 标准路径）**：5 招全部走"InspectScreen (I) 拖到 1-9 SkillBar 槽 → 战斗时按数字键 cast"统一路径，**不走右键左键鼠标键位**。
  - **触发**：按数字键 → server 创建 `Casting { duration_ticks, start_position }` component → cast bar 显示进度
  - **释放方向**：cast 完成瞬间读取玩家 look 作为弹道方向（不是 cast 开始时锁定）—— mastery 高时 HUD 准星收紧动画反馈"瞄得稳"，物理上方向仍以释放瞬间为准
  - **中断**：移动超 `Casting.start_position` 阈值 / 受击伤害超阈值 → server 移除 `Casting` 组件，载体不消耗、冷却走中断短冷却（plan-hotbar-modify-v1 §3.3）
  - **载体消耗**：cast 完成时按 `bound_instance_id` 扣 hand-slot 持骨 / 容器活跃槽载体；中断 → 不扣
  - **持骨前置**：① ② ③ ④ ⑤ cast 前 server 端校验 hand-slot 是否持有匹配档位载体（无载体 → cast 失败 + HUD 红字提示），不需玩家手动"持骨 → 瞄准"两步
  - 各招"操作"段只写**差异**（duration / HUD 反馈 / release 是 raycast 还是多弹道），通用部分不重复

---

## §1 五招完整规格

### ①「单射狙击」(v1 已实装，v2 迁出至 anqi_v2 主模块 + mastery 扩展)

**境界要求**：无 gate（醒灵 → 化虚都能学，威力随境界）

**真元消耗**：20-30% qi_max（载体封存比例，载体档位决定）

**冷却**：3.0s → 0.8s（mastery 0→100）

**伤害**：
- 基础：load × decay_factor × W_factor（v1 公式不动）
- 醒灵 + 档 1 残骨：5-12（30 格脱靶）
- 化虚 + 档 6 上古残骨：80-180（150 格保留 90%）

**操作**（差异）：cast time 短（瞬发型，duration 0.3s → 0.1s 跟 mastery）→ cast 完成单弹道 raycast → 飞行衰减 → 命中注射；HUD 准星收紧动画跟 mastery 缩紧速度。v1 已实装的 hand-slot 持骨在 cast 前 server 端自动校验

**mastery 生长**：
- 准星收紧速度 +50%（mastery 0→100）
- 飞行轨迹扰动幅度 -50%（直线度提升）
- 命中暴击概率 0% → 15%（mastery 100）

**经脉依赖**（接 plan-meridian-severed-v1）：手三阴之一 `Lung` / `Heart` / `Pericardium`（worldview §593 手三阴偏气、利远程；封元基础），SEVERED 任一 → ① 失效

**测试饱和**：保留 v1 已写测试 + 添加 mastery 增长 / 暴击 5 单测

---

### ②「多发齐射」(v2 新增)

**境界要求**：无 gate

**真元消耗**：每根载体独立封存（一次释放消耗 5 根档 3 灵木箭载体，每根 8% qi_max → 总 40% qi_max；蓄力 1.5s）

**冷却**：12s → 4s（mastery 0→100）

**伤害**：
- 5 弹道扇形 60° 散射 / 距离 30 格
- 单弹道：3-7（档 3 灵木箭半凝色 ρ=0.5）
- 暴击 / 命中数随 mastery：mastery 0 命中 1-2 / mastery 100 命中 3-5

**操作**（差异）：duration 1.5s → 0.6s（mastery 0→100）；cast 期间 HUD 显示扇形预览（散射角度收敛动画跟 mastery）+ 准星收紧；cast 完成自动 release 5 弹道独立 raycast（释放瞬间 look 决定中心方向）

**mastery 生长**：
- 散射角度 60° → 30°（更聚拢，命中率↑）
- 单弹道追踪偏角 0° → 5°（轻微制导）
- 蓄力时间 1.5s → 0.6s
- 命中数上限 +1（顶 5）

**经脉依赖**：手厥阴心包经 `Pericardium`（worldview §593 手三阴之一，统血协调多源同步——5 弹道齐发需要心包经统协气血输出），SEVERED → ② 失效

**测试饱和**：cone_dispersion 算子 5 单测（角度 / 距离 / 命中数）+ 5 弹道独立 raycast 5 单测 + mastery 增长 5 单测

---

### ③「凝魂注射」(v2 新增)

**境界要求**：凝脉+ 威力门坎（醒灵 / 引气 cast 不出来，因为 ρ=0.2 凝实色门坎需要凝脉级真元控制）

**真元消耗**：30-50% qi_max（高密度封存，>30% 触发战后虚脱）

**冷却**：18s → 6s（mastery 0→100）

**伤害**：
- 单弹道（同 ① 单射）+ 命中后注射高密度真元
- 注射 wound：base × 1.5（凝实色 ρ=0.2 高密度增伤）
- 注射 contam：load × 0.3（凝实色 contam 转化）
- 凝实色匹配（攻方 `Cultivation.qi_color` ≈ 载体 `qi_color`）→ 伤害 ×1.3
- 不匹配 → 伤害 ×0.6

**操作**（差异）：duration 1.0s → 0.4s（mastery 0→100）；cast 期间 HUD 凝魂封存圈缩小动画（凝实色光晕逐步聚拢） + 准星收紧；cast 完成单弹道 raycast 注射高密度真元

**mastery 生长**：
- 凝魂封存时间 1.0s → 0.4s
- 凝实色匹配阈值放宽（mastery 100 允许 ±2 染色档差仍按"匹配"）
- 注射 wound 增益 1.5 → 1.8
- 战后虚脱减免：>30% qi_max 注射，mastery 100 时虚脱 ticks 砍半

**经脉依赖**：足太阴脾经 `Spleen`（worldview §595 足三阴偏韧；中医"水谷精微"由脾运化，凝实色高密度封存依赖脾经精微输布），SEVERED → ③ 失效

**测试饱和**：high_density_inject 算子 5 单测（density 阈值 / wound 公式 / contam 转化）+ 凝实色匹配 / 不匹配 5 单测 + 战后虚脱触发 / 减免 3 单测

---

### ④「破甲注射」(v2 新增)

**境界要求**：固元+ 威力门坎（穿透 75% 防御，需要固元级真元推送）

**真元消耗**：40-60% qi_max（超功率封存，载体即时碎裂概率 50%）

**冷却**：25s → 10s（mastery 0→100）

**伤害**：
- 单弹道 + 穿透 75% 目标防御（armor_penetrate 算子）
- 基础：base × 1.8（穿透增益）
- 载体即时碎裂（mastery 0：50% / mastery 100：15%）→ 真元浪费但伤害仍命中
- 化虚级 + 档 5 封灵匣骨：穿透 90% 防御 + base × 2.5

**操作**（差异）：duration 2.0s → 0.8s（mastery 0→100）；cast 期间 HUD 显示载体共振警告（裂纹动画 + 准星红色震动）；cast 完成单弹道 raycast，载体即时碎裂 50%→15%（mastery）→ 飞溅特效；mastery 100 受击中断阈值放宽（允许受 10+ 真元损伤仍持续 cast）

**mastery 生长**：
- 蓄力时间 2.0s → 0.8s
- 载体碎裂概率 50% → 15%
- 穿透防御 75% → 90%（仅 mastery 100 + 化虚级）
- 蓄力中断条件放宽（mastery 100 允许受 10+ 真元损伤仍蓄力）

**经脉依赖**：手阳明大肠经 `LargeIntestine`（worldview §594 手三阳偏力、利近战爆发；超功率封存推送依赖大肠经的"力"属性），SEVERED → ④ 失效

**测试饱和**：armor_penetrate 算子 5 单测（穿透防御 / 化虚级特殊路径）+ 载体碎裂概率 / 概率分布 5 单测 + 蓄力中断 5 单测

---

### ⑤「诱饵分形」(化虚专属 / v2 新增)

**境界要求**：**化虚 gate**（仅化虚境玩家解锁；其他境界 cast 失败提示"未达化虚"）—— 跟 zhenmai-v2 ⑤ 绝脉断链 / baomai-v3 ⑤ 散功 化虚专属同列

**真元消耗**：50-70% qi_max（超量封存触发分形场）

**冷却**：300s → 120s（mastery 0→100）

**伤害**：
- 单根档 6 上古残骨 → 化虚境真元浓度场扰动 → 分裂 30+ echo（mastery 100 + 高密度区 90+ echo）
- 单 echo 伤害：(base × 2.0) / N（总伤害一致，不是叠加）
- 单 echo 独立 raycast，可被范围伤害挡住
- 命中目标后载体本体即时碎裂（一根载体一次分形）

**物理推导**：见 §0 化虚级诱饵分形段，echo 数 = floor(local_qi_density / threshold)

**操作**（差异）：duration 3.0s → 1.2s（mastery 0→100）；cast 期间 HUD 显示分形场预热动画（化虚境真元浓度场涟漪 ripple + echo 数预测计数从 0 累加 + 准星收紧）；cast 完成 release 30+ 弹道扇形飞出（释放瞬间 look 决定中心方向，每 echo 独立 raycast）

**mastery 生长**：
- 分形场阈值 0.3 → 0.1（mastery 100 echo 数 ×3）
- 单 echo 伤害 (base × 2.0) → (base × 2.5)
- 蓄力时间 3.0s → 1.2s
- echo 飞行扰动幅度 ±15° → ±5°（更聚拢）

**经脉依赖**：督脉 `Du`（worldview §597 任督=统御，化虚境奇经全通后激活的"阳脉之海"，作为化虚 echo 分形的真元感应+广播枢纽——worldview §201 "化虚后 20 经脉全开"明示无第 21 条特殊脉，化虚专属能力归到督脉），SEVERED → ⑤ 失效（督脉 SEVERED = 化虚境退化为通灵级，跨周目永久残废）

**反 MMO 红线**：每个 echo 是真实碰撞实体，**不是无敌分身**：
- 涡流涡心紊流场覆盖区可挡 echo
- 截脉多点反震可反震 echo（每个 echo 反震独立结算）
- 体修撼山可冲散 echo
- 30 echo 全脱靶 → 真元浪费 + 载体半衰跳过 + 战后虚脱

**触发天道注视**（接 plan-tribulation-v1）：echo 数 ≥ 30 触发天道注视累积（worldview §三:78 化虚天道针对，跟 zhenmai-v2 绝脉断链化虚级同列）

**测试饱和**：density_echo 算子 5 单测（echo 数 / 浓度阈值 / mastery 影响）+ 单 echo 独立碰撞 5 单测 + 范围伤害挡 echo 5 单测 + 化虚 gate 拒绝 3 单测 + 天道注视触发 3 单测

---

## §2 经脉依赖（接 plan-meridian-severed-v1）

暗器流派依赖经脉清单：

| 经脉（代码 enum） | 招式依赖 | SEVERED 来源 | SEVERED 后果 |
|---|---|---|---|
| 手三阴之一 `Lung` / `Heart` / `Pericardium` | ① 单射狙击 | CombatWound（贴脸近战）/ DuguDistortion / Other | 三条全断 → 暗器流派几乎全废；断一条 → 单射衰减 |
| 手厥阴心包经 `Pericardium` | ② 多发齐射 | OverloadTear（多源同步过载）/ CombatWound | ② 失效（仅剩单发）；玩家未通 `Pericardium` 也无法 cast ② |
| 足太阴脾经 `Spleen` | ③ 凝魂注射 | OverloadTear（>30% 注射触发）/ DuguDistortion / TribulationFail | 凝实色匹配机制失效 |
| 手阳明大肠经 `LargeIntestine` | ④ 破甲注射 | OverloadTear（>40% 注射）/ CombatWound | 穿甲能力丧失 |
| 督脉 `Du` | ⑤ 诱饵分形 | TribulationFail（化虚雷劫炸）/ DuguDistortion / VoluntarySever | 化虚境暗器师退化为通灵级（跨周目永久残废） |

**v2 实装**：在 plan-meridian-severed-v1 的 7 流派经脉依赖表中追加暗器条目；P3 阶段交付。

---

## §3 craft-v1 配方接入（6 档载体 + 3 容器）

接 `plan-craft-v1` 🟡，AnqiCarrier 类目添加 6 档载体 + Container 类目添加 3 容器：

| 配方 ID | 类目 | 输出 | 输入 | 时间 | 解锁渠道 |
|---|---|---|---|---|---|
| `anqi.carrier.bone_chip` | AnqiCarrier | 残骨 ×3 | 普通骨 ×1 | 1 min | 默认（残卷） |
| `anqi.carrier.mutant_bone` | AnqiCarrier | 异变兽骨 ×1 | 异变兽核 ×1 + 普通骨 ×3 | 8 min | 残卷（缝合兽掉） |
| `anqi.carrier.lingmu_quiver` | AnqiCarrier | 灵木编制箭 ×5 | 灵木 ×3 + 凝实色染料 ×1 | 12 min | 师承 |
| `anqi.carrier.dyed_bone` | AnqiCarrier | 染色骨 ×1 | 异变兽骨 ×1 + 凝实色染料 ×3 + 草药 ×5 | 30 min | 师承 |
| `anqi.carrier.fenglinghe_bone` | AnqiCarrier | 封灵匣骨 ×1 | 染色骨 ×1 + 上古残骨碎 ×1 | 60 min | 顿悟 |
| `anqi.carrier.shanggu_bone` | AnqiCarrier | 上古残骨 ×1 | 上古残骨碎 ×3 + 化虚级真元注 | 120 min | 顿悟（化虚级专属） |
| `anqi.container.quiver` | Container | 箭袋 | 兽皮 ×3 + 凝实色染料 ×1 | 5 min | 默认 |
| `anqi.container.pocket_pouch` | Container | 裤袋 | 兽皮 ×1 + 灵木 ×1 | 2 min | 默认 |
| `anqi.container.fenglinghe` | Container | 封灵匣 | 异兽骨骼 ×3 + 灵木 ×5 | 30 min | 师承 |

**v2 实装**：P0 阶段把这些配方写进 craft-v1 配方表（不是 plan 内自己实装 craft 系统）。

---

## §4 多容器系统

接 v1 vN+1 留待问题"多容器"：

| 容器 | 槽位 | 入囊磨损税 | 出囊磨损税 | 特性 |
|---|---|---|---|---|
| Hand-slot | 1 | 0%（v1 已实装） | 0% | 持骨于手不入囊 |
| 箭袋（quiver） | 12 | 5% qi 损失 / 入 | 5% qi 损失 / 出 | 多容量，入 / 出快 |
| 裤袋（pocket pouch） | 4 | 8% qi 损失 / 入 | 8% qi 损失 / 出 | 隐蔽，不显眼（worldview §K narration 沉默） |
| 封灵匣（fenglinghe） | 6 | 0% qi 损失 / 入 | 0% qi 损失 / 出 | 不入箱使用不扣次数（worldview §十一:1416）；但不可在战斗中切换 |

**磨损税公式**（走 `qi_physics::container::abrasion_loss`）：
```
abrasion_loss = carrier.qi_payload × tax_rate
qi_payload -= abrasion_loss
（损失的真元归还 zone，符合守恒律 plan-qi-physics-v1）
```

**容器切换 UX**：F 键切换 → HUD 容器栏切换动画 → 切换中 0.5s 暴露窗口（不可释放）

**v2 实装**：P2 阶段交付。

---

## §5 客户端动画 / VFX / SFX

| 招式 | 动画 | 粒子 | 音效 |
|---|---|---|---|
| ① 单射 | v1 已有（举臂瞄准 → 投掷） | v1 已有（载体飞行轨迹） | v1 已有（嗖 / 命中骨裂） |
| ② 齐射 | 双手散花姿态（蓄力 → 扇形挥洒）| 5 弹道扇形飞行轨迹 + 共振涟漪 | 蓄力嗡 + 5 嗖（错峰）+ 命中骨裂 ×N |
| ③ 凝魂 | 单手凝指（凝实色光芒在指尖封存） | 凝实色高密度光团 + 命中后注射特效 | 凝魂嗡 + 嗖 + 注射湿润声 |
| ④ 破甲 | 反弓蓄力（载体共振警告 → 释放） | 载体共振裂纹 + 穿甲冲波 | 共振嗡 → 暴鸣 + 穿甲咔嚓 |
| ⑤ 分形（化虚） | 双手合掌 → 释放分形场 | 30+ echo 弹道扇形 + 化虚境真元浓度场扰动 ripple | 化虚分形嗡（持续 1s）+ 30+ 嗖（密集）+ 战场覆盖音效 |

HUD 组件（plan-HUD-v1 接入）：

- **载体格栅**（hand-slot + 容器活跃槽）：6 档载体图标 + 当前 qi_payload / max
- **容器栏**（hand / 箭袋 / 裤袋 / 封灵匣）：4 容器切换图标 + 槽位数显示
- **凝魂条**：凝魂注射蓄力进度（半圆环，凝实色）
- **诱饵 marker**：诱饵布设位置 ground marker（v2 P2 ④ 破甲注射相关）
- **echo 计数**：化虚分形释放时显示当前 echo 数 + 命中数实时累积
- **磨损 tooltip**：hover 容器栏时显示入 / 出囊预计 qi 损失
- **准星收紧动画**：cast 期间准星跟 mastery 缩紧速度（mastery 0 缓慢 / mastery 100 极快），cast 完成准星收最紧 → 释放瞬间读 look 决定弹道方向

**Dev 测试指令（client side，跟 `BongVfxCommand` / `BongAnimCommand` / `BongSpawnParticleCommand` 同模式）**：

新增 `client/src/main/java/com/bong/client/debug/BongHudCommand.java` —— 注册 `ClientCommandManager` `/bonghud <subcommand>`，独立测试 HUD 动画**不依赖真实 cast 流程**：

- `/bonghud aim_enclose <progress 0.0-1.0> [duration_ms]` —— 模拟准星收紧到指定进度（progress 1.0 = 收最紧；duration_ms 缺省 1000）
- `/bonghud aim_enclose mastery <0-100>` —— 用 mastery 反推收紧速度跑一次完整动画
- `/bonghud charge_ring <progress 0.0-1.0> [duration_ms]` —— 模拟蓄力进度环（凝魂条 / 破甲共振 / 分形场预热复用）
- `/bonghud echo_count <n>` —— 模拟化虚分形 echo 计数 HUD（n=30 / 60 / 90）
- `/bonghud abrasion_tooltip <container> <qi_payload>` —— 模拟磨损 tooltip 显示
- `/bonghud clear` —— 清掉所有 dev 触发的 HUD 模拟态

不发任何 server payload，纯 client side 渲染，便于美术 / dev 迭代 HUD 动画曲线（Bezier 缓动 / 弹性回弹 / linear 等）独立调参。

**v2 实装**：P4 阶段交付。

---

## §6 阶段交付物（P0 → P5）

### P0 — 6 档载体 + 多发齐射底盘（4-6 周）

- [ ] `combat::anqi_v2` 主模块（v1 单射代码迁入 + v2 招式骨架）
- [ ] `CarrierGrade` enum 6 档 + `AnqiCarrierKind` 添加 5 档新载体
- [ ] `craft-v1` 接入：6 档载体 + 3 容器配方
- [ ] `qi_physics::projectile::cone_dispersion` 算子（patch P3 加）
- [ ] `MultiShotEvent` 事件 schema + agent narration
- [ ] ② 多发齐射招式实装（含 5 弹道独立 raycast）
- [ ] 测试：齐射 25 单测 + 6 档载体 12 单测 + craft 18 单测

### P1 — 凝魂注射 + 破甲注射（4 周）

- [ ] `qi_physics::projectile::high_density_inject` + `armor_penetrate` 算子（patch P3 加）
- [ ] `QiInjectionEvent` schema
- [ ] ③ 凝魂注射 + ④ 破甲注射招式实装
- [ ] 凝实色匹配机制（读 `Cultivation.qi_color`，不是 plan-color-v1）
- [ ] >30% 注射触发战后虚脱（v1 Q44 留待问题落地）
- [ ] 测试：凝魂 25 单测 + 破甲 22 单测 + 战后虚脱触发 / 减免 8 单测

### P2 — 多容器系统 + 磨损税（3 周）

- [ ] `ContainerSlot` component（4 选 1）
- [ ] `qi_physics::container::abrasion_loss` 算子（patch P3 加）
- [ ] 4 容器切换 + HUD 容器栏
- [ ] 磨损税扣 qi_payload + 归还 zone（守恒律走 ledger::QiTransfer）
- [ ] 战斗中切换暴露窗口 0.5s
- [ ] 测试：4 容器切换 16 单测 + 磨损税公式 12 单测 + 守恒律单测 6 单测

### P3 — 经脉依赖 + 熟练度生长 + 化虚诱饵分形（5-6 周）

- [ ] 接 `plan-meridian-severed-v1` 7 流派经脉表追加暗器条目
- [ ] 5 招 SEVERED 失效路径
- [ ] 5 招 mastery 字段（0-100）+ cast 加 mastery 公式
- [ ] mastery 生长效果（准星 / 散射 / 蓄力 / echo 数 / 凝实色匹配阈值等）
- [ ] `qi_physics::field::density_echo` 算子（patch P3 加）
- [ ] ⑤ 诱饵分形招式实装（化虚 gate + 30+ echo）
- [ ] 天道注视触发（echo ≥ 30 → tribulation 累积）
- [ ] 测试：经脉依赖 25 单测 + mastery 30 单测 + 诱饵分形 28 单测

### P4 — 客户端 5 动画 / 4 粒子 / 5 音效 / 6 HUD（4 周）

- [ ] 5 招动画（②③④⑤ 新动画 + ① 优化）
- [ ] 4 粒子（齐射扇形 / 凝魂光团 / 破甲冲波 / 分形 ripple）
- [ ] 5 音效 recipe
- [ ] 7 HUD 组件（载体格栅 / 容器栏 / 凝魂条 / 诱饵 marker / echo 计数 / 磨损 tooltip / 准星收紧动画）
- [ ] `BongHudCommand` 客户端 dev 命令（`/bonghud aim_enclose | charge_ring | echo_count | abrasion_tooltip | clear`，跟 `BongVfxCommand` 同模式，独立测试 HUD 动画）
- [ ] 测试：客户端 client/test 5 招视觉回归 + HUD 集成测试 + `/bonghud` 各子命令 dev smoke 测试

### P5 — v2 收口（饱和测试 + agent narration + e2e 联调）（2-3 周）

- [ ] agent `tiandao::anqi_v2_runtime`（5 招 narration 全量）
- [ ] 化虚分形战场叙事 + 凝实色匹配叙事 + 入囊扣真元提示
- [ ] e2e 联调：client → server cast → 弹道飞行 → 命中事件 → client 渲染（每招独立 e2e）
- [ ] 饱和测试 audit：5 招每招 ≥20 单测，总单测 ≥150
- [ ] 与 plan-style-balance-v1 对接：暗器 W/ρ 矩阵填表（vs 7 流派 7 个 W 数值）
- [ ] Finish Evidence + 迁入 docs/finished_plans/

---

## §7 已知风险 / open 问题

- [ ] **Q1** 化虚诱饵分形 echo 数上限：mastery 100 + 高密度区 = 90 echo？还是 60？性能 / 平衡 / 物理推导哪个优先 → P3 拍板（性能 profiling 后）
- [ ] **Q2** 凝实色匹配阈值放宽：mastery 100 ±2 染色档差按"匹配"——是不是过于强了 → P1 调参
- [ ] **Q3** 破甲注射载体即时碎裂概率 50%：是否吃载体档位 buff（档 6 上古残骨碎裂概率减半）→ P1 拍板
- [ ] **Q4** 多容器战斗中切换暴露窗口 0.5s：是否给 mastery 减免 → P2 拍板
- [ ] **Q5** 诱饵分形与替尸三档伪皮的交互：上古伪皮（plan-tuike-v2）能挡 30 echo 吗？还是只挡 1 个 → P3 拍板（与 tuike-v2 联动）
- [ ] **Q6** 凝魂注射 vs 截脉局部中和（plan-zhenmai-v2 ②）：单点高密度注射在血肉派接触面分布前会被中和多少？→ P1 拍板（与 zhenmai-v2 联动）
- [ ] **Q7** 化虚级跨周目（plan-multi-life-v1）：督脉 `Du` SEVERED 是否跨周目继承？→ P3 与 multi-life 联动

---

## §8 进度日志

- 2026-05-06：骨架创建。承接 plan-anqi-v1 ✅ finished（PR + commit 2026-05-04）。v2 范围明确：6 档载体 + 多发齐射 + 凝魂 / 破甲注射 + 化虚诱饵分形 + 多容器 + 磨损税 + 经脉依赖 + 熟练度生长。化虚专属诱饵分形走 worldview §P 真元浓度场扰动物理推导（**不是无敌分身**，30 echo 真实碰撞可被范围伤害挡住）。物资派定调（钱包代价换战场远射），与体修血肉派 / 毒蛊脏真元 / 替尸物理免疫一次性区分。
- 2026-05-07：经脉依赖正名。原"中冲脉 / 精微脉 / 阳明脉 / 化虚通脉"四条编造名核查后**全部不在 `MeridianId` enum 标准 20 条经脉里**（`server/src/cultivation/components.rs:49-72` = 12 正经 + 8 奇经）。修订映射：② 中冲脉→`Pericardium`（中冲是心包经井穴非脉）/ ③ 精微脉→`Spleen`（水谷精微由脾运化）/ ④ 阳明脉→`LargeIntestine`（手三阳偏力）/ ⑤ 化虚通脉→`Du`（worldview §201 "化虚后 20 经脉全开"明示无第 21 条特殊脉，§597 任督=统御 → 化虚专属能力归督脉）。① 单射狙击的"手三阴之一"为 worldview §593 原话，保留。
- **2026-05-09**：升 active（`git mv docs/plans-skeleton/plan-anqi-v2.md → docs/plan-anqi-v2.md`）。触发条件：
  - **plan-qi-physics-patch-v1 ✅ finished**（PR #162，2026-05-08）—— ρ=0.30 + W 矩阵 + cone_dispersion / high_density_inject / armor_penetrate / density_echo / abrasion_loss 算子接入路径就位
  - **plan-anqi-v1 ✅** + **plan-craft-v1 ✅** + **plan-meridian-severed-v1 ✅** + **plan-tsy-loot-v1 ✅** —— 6 档载体 + 5 经脉依赖 + 异变兽骨掉落全前置 ✅
  - 用户 2026-05-09 拍板**音效/特效/HUD 区分硬约束 + 招式独立 icon**：5 招（单射狙击 / 多发齐射 / 凝魂注射 / 破甲注射 / 诱饵分形）cast 必须各自携带差异化 animation + particle + SFX + HUD 反馈（§5 已表格化，验收硬约束化）+ **hotbar/SkillBar 槽位 PNG icon 每招独立**（不同载体档位 + 容器选择也走差异化 icon —— 6 档载体 × 3 容器，即 "残骨单射" / "兽骨齐射" / "灵木凝魂" 等子状态各自 icon overlay；走 `client/.../hud/SkillSlotRenderer.java` + 新增 `LoadoutIconLayer`，资源走 `/gen-image item` 批量），化虚诱饵分形特殊处理（30 echo 视觉 = 主弹道 PNG + 半透 echo 副 sprite），P4/P5 验收必须含视觉/听觉差异化回归 + icon 显示回归 + 多容器切换暴露窗口的 HUD overlay 回归
  - 下一步：进 P0（单射狙击 P0 校准 + ρ=0.30 接入 + 6 档载体补完 + 多容器骨架），收口 §7 七决策门

## Finish Evidence

### 落地清单

- **server / qi_physics**：新增 `qi_physics::projectile::{cone_dispersion, high_density_inject, armor_penetrate}`、`qi_physics::field::density_echo`、`qi_physics::container::abrasion_loss`，并在 `qi_physics::mod` 导出，统一暗器 v2 的散射、注射、穿甲、分形和容器磨损入口。
- **server / combat**：新增 `combat::anqi_v2`，注册 5 招 `AnqiSkillId`、`AnqiMastery`、`ContainerSlot`、`MultiShotEvent` / `QiInjectionEvent` / `ArmorPierceEvent` / `EchoFractalEvent` / `CarrierAbrasionEvent` / `ContainerSwapEvent` / `DecoyDeployEvent`；`combat::carrier::CarrierKind` 扩为 6 档载体并追加 `InjectionKind`。`anqi_container_switch` 已接入 client request，F 键循环触发 `ContainerSwapEvent`，非 hand-slot 施放会按 `abrasion_loss` 发出 `CarrierAbrasionEvent` 并使用扣税后的 payload；施放前会校验匹配 charged carrier / imprint，避免空手生产暗器事件。
- **server / craft + items**：`server/assets/items/anqi.toml` 增加 6 档载体、charged 变体和 3 容器；`craft` 新增 `Container` 类目并注册 6 个载体配方 + 3 个容器配方。`spirit_quality_initial` 保持 item registry 合法范围 `0..=1`，并有 asset load 回归覆盖。
- **server / cultivation + schema bridge**：5 招 technique 注册到 known techniques / skill registry / 经脉依赖；新增 anqi-v2 Redis channel 常量、Rust IPC payload、`RedisOutbound::AnqiMultiShot` / `AnqiQiInjection` / `AnqiEchoFractal` / `AnqiCarrierAbrasion` / `AnqiContainerSwap`，并接入 `anqi_event_bridge`。
- **agent/schema + tiandao**：TS schema 增加 anqi-v2 channels、carrier kind、multi-shot / qi-injection / echo-fractal / abrasion / container-swap contracts，并登记 schema registry；`QiInjectionEventV1.target` 必填且接受 server `Option<String>` 序列化出的 `null`；`anqi_container_switch.to` 仅允许 combat-swappable 容器；`AnqiNarrationRuntime` 订阅 v1 + v2 暗器事件，提供 multi-shot、qi-injection、echo-fractal、container abrasion、container swap fallback narration。
- **client HUD + command**：新增 `AnqiHudState` / `AnqiHudStateStore` / `AnqiHudPlanner` / `LoadoutIconLayer` / `BongHudCommand`；`BongClient` 注册 `/bonghud`，`BongHudOrchestrator` 接入暗器 HUD，`QuickBarHudPlanner` 对 skill slot 优先渲染 `iconTexture`，无图标时保留文字 fallback；技能栏存在 `anqi.*` 技能时 F 键发送 `anqi_container_switch` 用于容器栏生产路径，否则透传原版副手键。

### 测试结果

- `cd server && cargo fmt --check`
- `cd server && cargo clippy --all-targets -- -D warnings`
- `cd server && cargo test`：3252 passed
- `cd server && cargo test -q anqi -- --nocapture`：16 passed
- `cd server && cargo test -q inventory::tests::loads_item_registry_from_assets -- --nocapture`：1 passed
- `cd agent && npm ci`
- `cd agent && npm run build`
- `cd agent && npm test -w @bong/schema`：334 passed
- `cd agent && npm test -w @bong/tiandao`：312 passed
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests "com.bong.client.combat.SkillBarKeyRouterTest" --tests "com.bong.client.network.ClientRequestProtocolTest" --tests "com.bong.client.network.ClientRequestSenderTest"`：BUILD SUCCESSFUL
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build`：BUILD SUCCESSFUL

### 回归锚点

- Rust：`anqi_v2` 覆盖 5 招注册 / 经脉阻断 / mastery / cooldown / physics 事件、容器切换暴露窗口、F 键循环入口对应的 swap 事件、非 hand-slot draw 磨损事件、缺少 charged carrier 时拒绝施放；`qi_physics` 覆盖散射、注射、穿甲、分形和磨损税；`craft` 覆盖 6 载体 + 3 容器注册；`inventory` 覆盖 anqi item asset 合法加载。
- TypeScript：schema generated-artifact check 未漂移；`anqi-narration.test.ts` 覆盖 v2 channel 订阅、`target: null` 的 qi-injection、echo fallback、abrasion fallback 合法 narration contract 和 malformed payload reject；schema 单测覆盖缺失 `target` 拒绝与 `fenglinghe` combat switch 拒绝。
- Client：`AnqiHudPlannerTest` 覆盖准星收紧、蓄力条、echo 计数、磨损 tooltip、越界 progress 钳制和 null 安全；`LoadoutIconLayerTest` 覆盖招式 icon、echo 副标记和空 entry fallback；`BongHudCommandTest` 覆盖 `/bonghud aim_enclose | charge_ring | echo_count | abrasion_tooltip | clear` 参数树；`SkillBarKeyRouterTest` 覆盖无暗器技能时 F 键透传、有 `anqi.*` 技能时发送容器切换；`ClientRequestProtocolTest` / `ClientRequestSenderTest` 覆盖 `anqi_container_switch` JSON 和 `fenglinghe` 客户端拒绝。
