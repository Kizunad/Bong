# Bong · plan-zhenfa-v2 · 骨架

地师·阵法流功法**5 类阵法补全包**：护龛阵 / 聚灵阵 / 欺天阵 / 幻阵 / **跨位面阵 化虚专属**。承接 `plan-zhenfa-v1` ✅ finished（commit b82b02a0；P0/P1 诡雷 + 警戒场已实装，14 测试通过）—— v2 引入**4 大类剩余 3 类**（护龛 / 聚灵 / 欺天，v1 已搁置 Q14/Q15 拍板）+ **幻阵**（4 大类的隐蔽变体附属）+ **化虚专属跨位面阵**（worldview §三:187 化虚 ×5 凡躯重铸 + plan-tsy-dimension 联动：化虚阵法师把阵法投射到子位面，跨位面信号传递）+ **欺天阵物理推导**（worldview §八:614-618 气运劫持——向天道广播假劫气权重，**不是 MMO 式无敌护盾**，是真元浓度场扰动天道感应器，被识破反噬）+ **聚灵阵天道阈值落地**（worldview §八:602-606，v1 Q14 留待问题落地）+ **5 阵完整规格**（护龛 / 聚灵 / 欺天 / 幻阵 / 跨位面阵）+ **熟练度生长二维划分**（境界=阵威上限 / mastery=布阵速度 / 持续时长 / 逆逸散效率），无境界 gate 只有威力门坎。

**世界观锚点**：`worldview.md §五.3 地师/阵法流`（line ~341-345：唯一灵龛防御 + 数时辰朽坏） · `§六:616 缜密色 — 阵法师真元有规律纹路`（诡雷威力+ + 识破他人阵法） · `§八:602-606 灵物密度阈值`（聚灵阵天道注视代价物理推导） · `§八:614-618 气运劫持·欺天阵`（"凡人畏惧天命，而我们伪造天命"——欺天阵向天道广播假劫气权重，**不是无敌**，是真元场扰动天道感应器，被识破反噬更狠） · `§十三 末法无传送`（跨位面阵不是传送，是真元投影——位面间信号传递，非物质传送） · `§三:187 化虚 ×5 凡躯重铸`（化虚级跨位面阵物理推导前提） · `§K narration 沉默`

**library 锚点**：`peoples-0006 战斗流派源流` 攻击三·地师/阵法流原文 · `cultivation-0002 烬灰子内观笔记 §三·论影`（方块刻镜印的物理依据） · `ecology-0002 末法药材十七种`（夜枯藤 — 诡雷绝佳载体） · `ecology-0004 灵物磨损笔记`（载体磨损 = 阵法朽坏物理共源）

**前置依赖**：

- `plan-zhenfa-v1` ✅ → 诡雷 + 警戒场 + ZhenfaRegistry + 阵旗权限 + 阵眼实体 + 奖励 item 接口（v2 在此基础上扩 3 类 + 幻阵 + 化虚跨位面）
- `plan-qi-physics-v1` P1 ship → 阵法真元逆逸散走 `qi_physics::field::inverse_diffusion` 🆕（patch P3 加）+ 跨位面投影走 `qi_physics::field::cross_dimension_projection` 🆕（patch P3 加）
- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 W 矩阵（阵法 ρ=0.40 / W vs 4 攻 [0.5, 0.4, 0.6, 0.3]）+ inverse_diffusion / cross_dimension_projection 算子
- `plan-craft-v1` 🟡 → ZhenfaTrap 类目（5 类阵预埋件配方）—— v2 P0 必须先用此底盘
- `plan-meridian-severed-v1` 🆕 active → 阵法流派依赖经脉清单（任督二脉 + KI 足少阴肾经）+ SEVERED 阵法失效路径
- `plan-tsy-dimension-v1` ✅ finished → 子位面已有 server-data，化虚跨位面阵投射目标位面
- `plan-tribulation-v1` ⏳ → 聚灵阵天道注视累积 + 欺天阵被识破反噬（劫期权重操控反噬）
- `plan-narrative-political-v1` ✅ active → 欺天阵 + 化虚跨位面阵江湖传闻
- `plan-social-v1` ✅ → 灵龛归属（护龛阵需要灵龛主人身份认证）
- `plan-skill-v1` ✅ + `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅
- `plan-multi-style-v1` ✅ → 缜密色 PracticeLog
- `plan-anqi-v1` ✅ → 共享 `CarrierImprint` 字段（v1 双 plan 已对齐）
- `plan-cultivation-canonical-align-v1` ✅ → Realm + 经脉拓扑

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 阵的 W/ρ 数值进矩阵（阵法 ρ=0.40 / 专克毒蛊 W=0.6 因聚灵阵反污染 / 失效 vs 涡流 W=0.3 因紊流场冲散阵眼）
- `plan-tribulation-v1` ⏳ → 化虚跨位面阵 + 欺天阵化虚级触发天道注视（同 zhenmai-v2 / baomai-v3 / anqi-v2 化虚级同列）
- `plan-multi-life-v1` ⏳ → 跨周目阵法持久化处理（化虚阵法师跨周目还能维护吗？）
- `plan-yidao-v1` 🆕 placeholder → 护龛阵 + 医者结合（医者诊所被护龛阵保护路径）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation` / `cultivation::MeridianSystem` / `qi_physics::field::*`（逆逸散、跨位面投影、密度场）/ `qi_physics::ledger::QiTransfer`（阵法真元封存守恒律）/ `combat::carrier::CarrierImprint`（v1 已实装，v2 扩 `array_kind` 字段）/ `inventory`（载体素材 + 阵旗）/ `craft::CraftRegistry`（5 类阵预埋件配方）/ `social::ShrineOwnership`（灵龛归属）/ `tsy::DimensionRegistry`（子位面）/ `agent::tiandao::TribulationWeights`（欺天阵广播目标）/ `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm`
- **出料**：5 阵 `ZhenfaArrayKind` enum 注册到 ZhenfaRegistry（v1 已建立，v2 扩 5 个变体）/ `WardArrayDeployEvent` 🆕（护龛布阵）/ `LingArrayDeployEvent` 🆕（聚灵布阵 + 天道注视累积）/ `DeceiveHeavenEvent` 🆕（欺天阵广播假劫期权重）/ `IllusionArrayDeployEvent` 🆕（幻阵覆盖现有阵法）/ `CrossDimensionArrayEvent` 🆕（化虚跨位面投射）/ `ArrayDecayEvent` 🆕（5 类阵朽坏统一事件）/ `ArrayBreakthroughEvent` 🆕（被破解事件）
- **共享类型**：`ArrayImprint` component（继承 v1 `CarrierImprint`，扩 `dimension_target: Option<DimensionId>` + `tribulation_broadcast: bool` 字段）/ `ArrayMastery` component（mastery 0-100 + 5 阵各自的 lv 字段）
- **跨仓库契约**：
  - server: `combat::zhenfa_v2::*` 主实装（v1 诡雷 + 警戒场迁入 + v2 新 5 阵）/ `schema::zhenfa_v2`
  - agent: `tiandao::zhenfa_v2_runtime`（5 阵 narration + 化虚跨位面阵叙事 + 欺天阵被识破反噬叙事 + 聚灵阵天道注视提示） + `tiandao::tribulation_weights` 接受欺天阵广播
  - client: 5 阵动画（布阵 / 触发 / 朽坏 / 破阵 / 跨位面投射）+ 5 粒子 + 5 音效 recipe + 7 HUD 组件（阵图布局 / 阵眼 marker / 朽坏倒计时 / 天道注视进度 / 假劫期广播指示 / 跨位面通道 / 经脉依赖灰显）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：阵法真元逆逸散走 `qi_physics::field::inverse_diffusion` 🆕 / 跨位面投影走 `qi_physics::field::cross_dimension_projection` 🆕 / 聚灵阵真元密度增益走 `qi_physics::field::density_amplifier` 🆕 / 欺天阵天道感应扰动走 `qi_physics::field::tiandao_signal_distort` 🆕（patch P3 全加） / 朽坏速率走 v1 已实装 `qi_physics::container::abrasion_loss`（共享 anqi-v2 magnitude 不同 tax_rate）/ **禁止 plan 内自己写阵法 / 朽坏 / 跨位面 / 天道扰动公式**

---

## §0 设计轴心

- [ ] **坐标派 / 信息派定调（worldview §五.3 + §六:616 + §八:614）**：阵法流派区别于其他 6 流派——
  - **暗器**：单根载体 + 单点封存
  - **截脉**：自身经脉 + 接触面分布
  - **毒蛊**：自身脏真元 + 持续渗透
  - **替尸**：物资载体 + 一次性吸收
  - **体修**：自身经脉过载
  - **涡流**：环境真元场扰动
  - **阵法**：**地形坐标 + 信号广播 + 长期场**（与瞬时招式 / 单点输出区分）
  - 物理代价：所有阵法都需要"阵眼方块" + 真元封存 + 时长内不能离阵眼太远（>50 格 → 阵眼脱离视野，朽坏速率 ×3）

- [ ] **5 阵完整范围（v2）**：
  ```
  ① 护龛阵     | 灵龛防御     | 反弹/阻挡场 + 信誉度认证      | worldview §五.3
  ② 聚灵阵     | 真元密度增益 | 真元逆逸散场 + 天道注视风险   | worldview §八:602
  ③ 欺天阵     | 信息战       | 向天道广播假劫期权重          | worldview §八:614
  ④ 幻阵       | 隐蔽变体     | 覆盖现有阵法的可见性谱          | 4 大类附属
  ⑤ 跨位面阵   | 化虚专属     | 跨位面信号投射（非传送）        | worldview §三:187 + tsy-dimension
  ```

- [ ] **化虚级跨位面阵物理推导（worldview §三:187 + §十三）**：阵法的"质变"在化虚级专属——
  ```
  正常阵法：单一位面，阵眼必须在 50 格内
  化虚级跨位面阵：
    阵眼在主位面 → 真元投影到子位面（plan-tsy-dimension 接入）
    跨位面通信带宽：根据 mastery 决定
      mastery 0   → 单方向信号（主→子）
      mastery 50  → 双向信号（主↔子）
      mastery 100 → 多位面同步（主 + 子1 + 子2 ...）
    使用场景：
      - 跨位面警戒场（子位面有人活动 → 主位面通知）
      - 跨位面聚灵阵（子位面浓度 + 主位面共振，互相增益）
      - 跨位面欺天阵（化虚阵法师把假劫期权重广播给多个子位面，伪造跨位面劫气信号）
    限制：
      - 不传送物质（worldview §十三 末法无传送）
      - 信号衰减（跨位面 80% / 跨双位面 60%）
      - 主阵眼朽坏 → 所有子阵眼一起朽坏
  ```
  - 哲学：化虚阵法师**用真元投影替代物质传送**，跟坐标派定调一致——空间坐标和信号广播是阵法的本质，化虚级把这个能力扩展到位面尺度

- [ ] **欺天阵反 MMO 红线（worldview §八:614 物理推导）**：
  - 欺天阵**不是无敌护盾**，是向天道广播"我不是值得劫的目标"的假信号
  - 物理依据：天道通过真元密度场感应"应劫之人"，欺天阵在阵法师周围创造伪劫气真元场，让天道误判
  - **被识破反噬**：天道每 tick roll 一次"识破概率"，被识破 → 反噬劫期权重 ×3（"伪造天命被发现，反招更大劫"）
  - 化虚级欺天阵：识破概率 0.5%/tick → 反噬 ×3 = 期望劫期 ×1.015 / tick（即化虚阵法师即使欺天，长期统计劫期不会变小，只是延后）
  - **不是免疫劫期**，是延后 + 转嫁（PvP 中可主动转嫁劫期权重给敌方，worldview §八:614 物理化身）

- [ ] **不走 hotbar（v1 已正典化）**：阵法流派**全部走物品交互 packet**，不占 SkillBarBindings 槽位
  - `bong:zhenfa/place_array`（持阵旗 + 方块右键预埋）
  - `bong:zhenfa/trigger_array`（持阵旗按"使用"键引爆）
  - `bong:zhenfa/configure_array`（持阵旗按 Shift+使用 键打开 ZhenfaLayoutScreen，在 UI 内配置阵参数）
  - `bong:zhenfa/cross_dimension_link`（化虚跨位面阵专用，持化虚阵旗在子位面方块右键创建链接）

- [ ] **熟练度生长二维划分（v2 通用机制）**：
  - **境界**：决定阵威上限（伤害 / 持续时长 / 信号广播范围 / 真元密度增益）
  - **熟练度 mastery (0-100)**：决定布阵速度（cast time）/ 朽坏延缓（持续 ×1→×3）/ 逆逸散效率 / 跨位面通信带宽
  - 5 阵各自有 `mastery: u8` 字段，cast 一次 +0.3 / 阵被触发 +1.0
  - 数值表见 §1 各阵规格

- [ ] **专属物理边界 = 长期场（与 anqi 单点 / zhenmai 瞬时痕迹 / dugu 30min 残留 / tuike 蜕落物 30min 区分）**：
  - 阵法朽坏期：30min - 数小时（载体材质决定，跟 anqi-v2 6 档载体共享 carrier_grade）
  - 化虚跨位面阵朽坏期：12 - 24h（载体必须是上古残骨 + 阵眼方块必须是上古残土遗存方块）

---

## §1 五阵完整规格

### ①「护龛阵」

**功能**：灵龛周围建反弹/阻挡场（worldview §五.3 唯一灵龛防御）

**境界要求**：无 gate（醒灵 → 化虚都能学，威力随境界）

**真元消耗**：持续烧真元 0.5/s（阵眼维持，离场则关闭）

**冷却**：布阵 cast time 8s → 3s（mastery 0→100）

**效果**：
- 反弹：进入灵龛 5 格内的攻击 50% 反弹（含真元伤害 + 物理伤害 + 阵法伤害）
- 阻挡：未授权玩家 / NPC 进入 5 格内 → 推开 + qi 烧伤 5/tick
- 授权：灵龛主人 + 信誉度 ≥80 的盟友自由通行（接 plan-social-v1）

**载体**：阵眼方块（任意自然方块）+ 1 把阵旗 + 5 个上古残土碎方块

**朽坏期**：12h（阵旗常驻消耗真元）

**经脉依赖**：任督二脉（阵眼维持），SEVERED → ① 失效

**化虚级专属**：阵威范围 5 → 15 格 + 反弹率 50% → 80%

**测试饱和**：护龛阵 25 单测（反弹 / 阻挡 / 授权 / 朽坏 / 灵龛主人验证）

---

### ②「聚灵阵」

**功能**：真元封进环境，常驻提升灵气（worldview §八:602-606 灵物密度阈值）

**境界要求**：无 gate

**真元消耗**：一次性 30-50% qi_max + 持续 0.2/s 阵眼维持

**冷却**：布阵 cast time 30s → 12s（mastery 0→100）

**效果**：
- 阵眼周围 20 格 zone qi 浓度提升 ×1.5（醒灵）→ ×3 （化虚）
- 提升修炼速度 / shelflife 衰减 / 灵田生长（worldview §八:602 物理化身）
- **天道注视累积**：浓度阈值 (zone_density × 1.5 > 6.0) 触发 tribulation 累积（plan-tribulation-v1）

**载体**：3-9 个阵眼方块（多阵眼覆盖更大范围）+ 9 个上古残土碎 + 1 把阵旗

**朽坏期**：6h（多阵眼按"最先朽坏"算）

**经脉依赖**：任督 + KI（足少阴肾经，阵法师调节真元逆逸散），SEVERED → ② 失效

**化虚级专属**：阵眼数 9 → 27（覆盖范围 60 × 60 格）+ 浓度倍率 ×3 → ×5（同时天道注视风险 ×5）

**测试饱和**：聚灵阵 28 单测（浓度 / 阵眼数 / 天道阈值 / 朽坏 / 与 lingtian 系统联动 / 与 shelflife 联动）

---

### ③「欺天阵」

**功能**：向天道广播假劫气权重（worldview §八:614-618 气运劫持）

**境界要求**：固元+ 威力门坎（醒灵 / 引气 / 凝脉 cast 不出来，因为天道感应器分辨低境界欺骗）

**真元消耗**：极高 80-100% qi_max + 罕见材料（5 个化虚级真元封存的载体）

**冷却**：300s → 120s（mastery 0→100，每次施展冷却 + 业力累积）

**效果**：
- 60s 内向天道广播假劫气权重：自身劫期权重 -50% / 选定目标劫期权重 +50%
- 应用场景：
  - PvP 渡劫干扰：把对方推上劫期巅峰
  - 逃避自身劫期：60s 窗口内不会被天道选作劫期目标
  - 嫁祸：让无关目标承受劫期
- **被识破概率**：0.5%/tick（化虚 0.2%/tick）→ 识破后反噬 ×3（业力 + 劫期权重反弹）

**载体**：1 个化虚级阵眼（上古阵眼方块）+ 5 个上古残骨封真元 + 1 把化虚阵旗

**朽坏期**：1h（一次施展即朽坏）

**经脉依赖**：任督 + KI + 心经（高级真元广播），SEVERED 任一 → ③ 失效

**化虚级专属（v2 P0 默认即化虚级）**：识破概率 0.5% → 0.2%/tick + 反噬倍率不变（×3）—— 化虚阵法师玩"长期欺天"赌识破

**反 MMO 红线**：见 §0 第 4 条 — 长期统计劫期不变小，只是延后 + 转嫁，不是免疫

**触发天道注视**：每次施展 +tribulation_weight × 1.5（worldview §八:614 物理化身）—— 跟其他流派化虚级同列

**测试饱和**：欺天阵 30 单测（识破概率 / 反噬触发 / 劫期权重转嫁 / 业力累积 / 化虚级数值差异 / PvP 嫁祸路径）

---

### ④「幻阵」

**功能**：4 大类的隐蔽变体（覆盖现有阵法的可见性谱）

**境界要求**：无 gate（独立熟练度 mastery）

**真元消耗**：基础 10-20% qi_max（独立 + 现有阵法叠加）

**冷却**：cast time 5s → 2s（mastery 0→100）

**效果**：
- 覆盖在已布的诡雷 / 警戒场 / 护龛 / 聚灵 / 欺天阵上
- 隐藏阵法可见性：
  - 神识扫描需要 mastery_diff > 30 才能识破（识破 = 缜密色识色法 worldview §六:616）
  - 高境界识破阈值 -10（境界差自动加成）
- 不影响阵法效果，仅影响信息战

**载体**：1 把阵旗 + 1 张幻阵符（craft-v1 配方）

**朽坏期**：跟随被覆盖的阵法朽坏（"幻阵附属于本体"）

**经脉依赖**：KI（足少阴肾经，调控真元纹路），SEVERED → ④ 失效（不影响本体阵法）

**化虚级专属**：识破阈值 30 → 50 + 隐藏对方"阵法 mastery"显示

**测试饱和**：幻阵 20 单测（识破概率 / 缜密色 mastery 比对 / 与各类阵的覆盖 / 化虚级隐藏深度）

---

### ⑤「跨位面阵」(化虚专属 / v2 新增)

**功能**：跨位面信号投射（非传送，worldview §十三）

**境界要求**：**化虚 gate**（仅化虚境玩家解锁）—— 跟其他流派化虚专属同列

**真元消耗**：100-150% qi_max（超量封存触发位面投影；可分多次充能 30s/次）

**冷却**：300s → 60s（mastery 0→100）

**效果**：
- 主位面阵眼 → 子位面阵眼镜像投影（plan-tsy-dimension 联动）
- 跨位面通信带宽随 mastery：
  - mastery 0：单向（主→子）
  - mastery 50：双向（主↔子）
  - mastery 100：多位面同步（主 + 子1 + 子2 + 子3）
- 衰减：信号衰减跨位面 80% / 跨双位面 60% / 跨三位面 30%
- 应用：
  - 跨位面警戒场：子位面有人活动 → 主位面通知
  - 跨位面聚灵阵：浓度互相增益（mastery 100 总浓度 ×4）
  - 跨位面欺天阵：化虚阵法师向多位面广播假劫期权重

**载体**：1 个上古阵眼方块（主位面）+ 1-3 个上古阵眼方块（子位面）+ 5 个上古残骨封真元 + 1 把化虚阵旗

**朽坏期**：12h - 24h（载体载体材质决定，跟 anqi-v2 6 档载体共享 carrier_grade）

**经脉依赖**：督脉 `Du`（worldview §201 "化虚后 20 经脉全开"明示无第 21 条特殊脉，§597 任督=统御；督脉是化虚境扩展信号广播带宽到跨位面的载体——跟 anqi-v2 ⑤ / yidao-v1 ⑤ 化虚级同源，督脉同时承担 ①②③ 阵眼广播与 ⑤ 跨位面投射），SEVERED → ⑤ 失效（同时 ①②③ 信号广播功能受损；跨周目永久残废）

**触发天道注视**（接 plan-tribulation-v1）：跨位面投射 +tribulation_weight × 2.0（化虚级最重的注视累积）

**反 MMO 红线**：跨位面阵眼**不传送物质**（worldview §十三）—— 仅信号投射，不允许物质 / 玩家 / 物品跨位面移动

**测试饱和**：跨位面阵 32 单测（信号衰减 / mastery 带宽 / 多位面同步 / 主阵眼朽坏连锁 / 督脉 `Du` 依赖 / 天道注视触发）

---

## §2 经脉依赖（接 plan-meridian-severed-v1）

阵法流派依赖经脉清单：

| 经脉 | 阵依赖 | SEVERED 来源 | SEVERED 后果 |
|---|---|---|---|
| 任脉 `Ren` | ①②③⑤ 阵眼维持 | OverloadTear（聚灵阵超浓度过载）/ TribulationFail / DuguDistortion | 阵法流派几乎全废 |
| 督脉 `Du` | ①②③⑤ 真元广播（化虚级⑤扩展为跨位面带宽） | OverloadTear / TribulationFail（化虚雷劫炸）/ DuguDistortion / VoluntarySever | 信号广播能力丧失；化虚境阵法师退化为通灵级（跨周目永久残废） |
| 足少阴肾经 `Kidney` | ②④⑤ 真元逆逸散调控 | DuguDistortion（脏真元蚀脉）/ CombatWound | 聚灵 / 幻阵 / 跨位面阵失效 |
| 手少阴心经 `Heart` | ③ 高级真元广播 | OverloadTear（欺天阵被识破反噬）/ TribulationFail | 欺天阵失效 |

**v2 实装**：在 plan-meridian-severed-v1 7 流派经脉表追加阵法条目；P3 阶段交付。

---

## §3 craft-v1 配方接入（5 阵预埋件 + 阵旗）

接 `plan-craft-v1` 🟡，ZhenfaTrap 类目添加 5 阵专属预埋件 + 4 档阵旗：

| 配方 ID | 类目 | 输出 | 输入 | 时间 | 解锁渠道 |
|---|---|---|---|---|---|
| `zhenfa.array.ward` | ZhenfaTrap | 护龛阵预埋件 | 上古残土碎 ×5 + 阵眼方块 ×1 | 15 min | 残卷 |
| `zhenfa.array.lingju` | ZhenfaTrap | 聚灵阵预埋件 | 上古残土碎 ×9 + 阵眼方块 ×3 + 灵泉水 ×3 | 30 min | 师承 |
| `zhenfa.array.deceive` | ZhenfaTrap | 欺天阵预埋件 | 上古残骨 ×5 + 阵眼方块 ×1 + 真元封存 80% qi_max | 60 min | 顿悟（固元+） |
| `zhenfa.array.illusion` | ZhenfaTrap | 幻阵符 ×3 | 缜密色染料 ×3 + 灵木 ×2 | 5 min | 师承 |
| `zhenfa.array.cross_dim` | ZhenfaTrap | 跨位面阵预埋件 | 上古阵眼方块 ×3 + 上古残骨 ×5 + 化虚级真元注 | 120 min | 顿悟（化虚专属） |
| `zhenfa.flag.basic` | Tool | 基础阵旗 | 灵木 ×2 + 兽皮 ×1 + 缜密色染料 ×1 | 8 min | 默认 |
| `zhenfa.flag.deceive` | Tool | 欺天阵旗 | 上古残骨碎 ×3 + 真元封存 50% qi_max | 30 min | 顿悟（固元+） |
| `zhenfa.flag.cross_dim` | Tool | 化虚阵旗 | 上古阵眼方块 ×1 + 化虚级真元注 | 60 min | 顿悟（化虚专属） |

**v2 实装**：P0 阶段把这些配方写进 craft-v1 配方表（不是 plan 内自己实装 craft 系统）。

---

## §4 客户端动画 / VFX / SFX

| 阵 | 布阵动画 | 触发 / 效果粒子 | 朽坏粒子 | 音效 |
|---|---|---|---|---|
| ① 护龛阵 | 持旗围灵龛走 5 步（缜密色光纹画圆） | 反弹光晕 + 阻挡推力波 | 缜密色灰飞 | 咏唱 + 反弹叮 + 阻挡嗡 |
| ② 聚灵阵 | 持旗布 3-9 阵眼（每点缜密色立柱） | 真元密度场涟漪 + 灵气可视化 | 阵柱倒塌 | 咏唱 + 灵气嗡（持续） |
| ③ 欺天阵 | 持化虚阵旗对天画符 | 假劫气云团升起（向天道方向）+ 识破时反噬雷光 | 阵眼熔毁 | 咏唱 + 假劫云嗡 + 反噬雷鸣（识破） |
| ④ 幻阵 | 持旗甩出符纸覆盖现有阵法 | 缜密色雾化覆盖 + 神识扫到时显形涟漪 | 雾散 | 符纸碎裂 + 显形咔嚓 |
| ⑤ 跨位面阵 | 持化虚阵旗多次施法（主位面 + 子位面共 N 次） | 跨位面通道光柱 + 信号传递可视化 | 通道关闭 | 化虚共振嗡 + 跨位面信号嗡 |

HUD 组件（plan-HUD-v1 接入）：

- **阵图布局 UI**（持旗 Shift+使用键）：5 阵图标 + 阵眼布置预览 + 阵参数配置（载体材质 / 阵威 / 朽坏期）
- **阵眼 marker**：地图上显示已布阵眼位置 + 朽坏倒计时
- **天道注视进度**：聚灵 / 跨位面阵布阵时显示当前 tribulation_weight（圈环）
- **假劫期广播指示**：欺天阵激活时显示 "60s 假劫期生效中" + 识破概率实时显示
- **跨位面通道**：化虚阵法师 HUD 显示跨位面活跃通道列表 + 各通道信号衰减
- **经脉依赖灰显**：cast 阵法前 HUD 灰显被 SEVERED 的阵法
- **缜密色识破工具**：神识扫描他人阵法时显示识破概率（mastery_diff 决定）

**v2 实装**：P4 阶段交付。

---

## §5 阶段交付物（P0 → P5）

### P0 — 护龛阵 + 聚灵阵 底盘（4-6 周）

- [ ] `combat::zhenfa_v2` 主模块（v1 诡雷 + 警戒场迁入 + v2 阵骨架）
- [ ] `ZhenfaArrayKind` enum 扩 5 个变体
- [ ] `craft-v1` 接入：5 阵预埋件 + 4 档阵旗配方
- [ ] `qi_physics::field::inverse_diffusion` + `density_amplifier` 算子（patch P3 加）
- [ ] `WardArrayDeployEvent` + `LingArrayDeployEvent` schema
- [ ] ① 护龛阵 + ② 聚灵阵实装
- [ ] 灵龛主人验证（接 plan-social-v1）
- [ ] 聚灵阵天道注视累积（接 plan-tribulation-v1，v1 Q14 留待问题落地）
- [ ] 测试：护龛 25 单测 + 聚灵 28 单测 + craft 16 单测

### P1 — 欺天阵 + 反 MMO 物理推导（4 周）

- [ ] `qi_physics::field::tiandao_signal_distort` 算子（patch P3 加）
- [ ] `DeceiveHeavenEvent` schema + agent 接受广播
- [ ] ③ 欺天阵实装（识破概率 + 反噬 + 业力累积）
- [ ] 劫期权重转嫁（PvP 嫁祸路径）
- [ ] agent::tiandao::tribulation_weights 接受 zhenfa 广播事件
- [ ] 测试：欺天阵 30 单测 + 识破 / 反噬 8 单测 + tribulation 接入 5 单测

### P2 — 幻阵 + 缜密色识破（3 周）

- [ ] `IllusionArrayDeployEvent` schema
- [ ] ④ 幻阵实装（覆盖在已布阵法）
- [ ] 缜密色识色法（接 plan-multi-style-v1 PracticeLog）
- [ ] 神识扫描时识破概率计算（mastery_diff）
- [ ] 测试：幻阵 20 单测 + 识破 8 单测 + 与各类阵覆盖 12 单测

### P3 — 经脉依赖 + 熟练度生长 + 化虚跨位面阵（5-6 周）

- [ ] 接 `plan-meridian-severed-v1` 7 流派经脉表追加阵法条目
- [ ] 5 阵 SEVERED 失效路径
- [ ] 5 阵 mastery 字段（0-100）+ cast/触发加 mastery 公式
- [ ] mastery 生长效果（cast time / 朽坏延缓 / 逆逸散效率 / 跨位面带宽）
- [ ] `qi_physics::field::cross_dimension_projection` 算子（patch P3 加）
- [ ] ⑤ 跨位面阵实装（化虚 gate + tsy-dimension 接入）
- [ ] 天道注视触发（化虚跨位面阵 +2.0 weight）
- [ ] 测试：经脉依赖 25 单测 + mastery 30 单测 + 跨位面阵 32 单测

### P4 — 客户端 5 动画 / 5 粒子 / 5 音效 / 7 HUD（4 周）

- [ ] 5 阵动画（布阵 / 触发 / 朽坏 / 破阵）
- [ ] 5 粒子 + 5 音效 recipe
- [ ] 7 HUD 组件（阵图布局 / 阵眼 marker / 天道注视 / 假劫期 / 跨位面通道 / 经脉依赖灰显 / 缜密色识破）
- [ ] 测试：客户端 5 阵视觉回归 + HUD 集成测试

### P5 — v2 收口（饱和测试 + agent narration + e2e 联调）（2-3 周）

- [ ] agent `tiandao::zhenfa_v2_runtime`（5 阵 narration 全量 + 跨位面阵 + 欺天阵识破反噬）
- [ ] 化虚跨位面阵江湖传闻 + 欺天阵被识破叙事
- [ ] e2e 联调：client → server cast → 阵法布置 → 触发 / 朽坏 / 跨位面投射 → client 渲染（每阵独立 e2e）
- [ ] 饱和测试 audit：5 阵每阵 ≥20 单测，总单测 ≥150
- [ ] 与 plan-style-balance-v1 对接：阵法 W/ρ 矩阵填表（vs 7 流派 7 个 W 数值）
- [ ] Finish Evidence + 迁入 docs/finished_plans/

---

## §6 已知风险 / open 问题

- [ ] **Q1** 化虚跨位面阵：mastery 100 多位面同步（主 + 子1 + 子2 + 子3）是否过强？性能 / 平衡 / 物理推导哪个优先 → P3 拍板
- [ ] **Q2** 欺天阵识破概率（0.5% / 0.2%）：是否需要根据天道权重动态调整 → P1 拍板
- [ ] **Q3** 聚灵阵天道注视阈值（zone_density × 1.5 > 6.0）：v1 留待 Q14 拍板，v2 需根据 lingtian 系统实测调参 → P0 拍板
- [ ] **Q4** 多个聚灵阵叠加：阵眼位置近 → 浓度叠加是否触发更高天道注视 → P0 拍板
- [ ] **Q5** 幻阵 vs 神识扫描：是否给被扫描者主动反馈"被扫描"信号 → P2 拍板
- [ ] **Q6** 化虚跨位面阵跨周目（plan-multi-life-v1）：化虚阵法师跨周目还能维护原阵眼吗？→ P3 与 multi-life 联动
- [ ] **Q7** 欺天阵嫁祸 PvP：被嫁祸玩家如何感知 + 反制路径 → P1 拍板（与 plan-narrative-political-v1 联动）

---

## §7 进度日志

- 2026-05-06：骨架创建。承接 plan-zhenfa-v1 ✅ finished（commit b82b02a0；P0/P1 诡雷 + 警戒场已实装）。v2 范围明确：4 大类剩余 3 类（护龛 / 聚灵 / 欺天）+ 幻阵 + 化虚跨位面阵 + 5 经脉依赖 + 熟练度生长 + 反 MMO 物理推导。化虚跨位面阵走 worldview §三:187 + §十三 末法无传送（**信号投射非物质传送**）。欺天阵走 worldview §八:614-618 气运劫持（**不是免疫**，是真元场扰动天道感应器，被识破反噬 ×3）。聚灵阵天道阈值落地（v1 Q14）。坐标派 / 信息派定调，与暗器单点 / 截脉接触面 / 毒蛊渗透 / 替尸钱包 / 体修肉体 / 涡流环境改造区分。
