# Bong · plan-zhenfa-v2 · 骨架

地师·阵法流功法**4 类阵法补全包**：护龛阵 / 聚灵阵 / 欺天阵 / 幻阵。承接 `plan-zhenfa-v1` ✅ finished（commit b82b02a0；P0/P1 诡雷 + 警戒场已实装，14 测试通过）—— v2 引入**4 大类剩余 3 类**（护龛 / 聚灵 / 欺天，v1 已搁置 Q14/Q15 拍板）+ **幻阵**（4 大类的隐蔽变体附属）+ **欺天阵物理推导**（worldview §八:614-618 气运劫持——zone 快照伪造欺骗天道 agent，被守恒校验识破反噬）+ **聚灵阵天道阈值落地**（worldview §八:602-606，v1 Q14 留待问题落地）+ **4 阵完整规格**（护龛 / 聚灵 / 欺天 / 幻阵）+ **熟练度生长二维划分**（境界=阵威上限 / mastery=布阵速度 / 持续时长 / 逆逸散效率），无境界 gate 只有威力门坎。~~跨位面阵砍掉（2026-05-10）：子位面实例化尚未实装，留 zhenfa-v3~~。

**世界观锚点**：`worldview.md §五.3 地师/阵法流`（line ~341-345：唯一灵龛防御 + 数时辰朽坏） · `§六:616 缜密色 — 阵法师真元有规律纹路`（诡雷威力+ + 识破他人阵法） · `§八:602-606 灵物密度阈值`（聚灵阵天道注视代价物理推导） · `§八:614-618 气运劫持·欺天阵`（"凡人畏惧天命，而我们伪造天命"——欺天阵向天道广播假劫气权重，**不是无敌**，是真元场扰动天道感应器，被识破反噬更狠） · `§K narration 沉默`

**library 锚点**：`peoples-0006 战斗流派源流` 攻击三·地师/阵法流原文 · `cultivation-0002 烬灰子内观笔记 §三·论影`（方块刻镜印的物理依据） · `ecology-0002 末法药材十七种`（夜枯藤 — 诡雷绝佳载体） · `ecology-0004 灵物磨损笔记`（载体磨损 = 阵法朽坏物理共源）

**前置依赖**：

- `plan-zhenfa-v1` ✅ → 诡雷 + 警戒场 + ZhenfaRegistry + 阵旗权限 + 阵眼实体 + 奖励 item 接口（v2 在此基础上扩 3 类 + 幻阵）
- `plan-qi-physics-v1` P1 ship → 阵法真元逆逸散走 `qi_physics::field::inverse_diffusion` 🆕（patch P3 加）- `plan-qi-physics-patch-v1` P0/P3 → 7 流派 W 矩阵（阵法 ρ=0.40 / W vs 4 攻 [0.5, 0.4, 0.6, 0.3]）+ inverse_diffusion / cross_dimension_projection 算子
- `plan-craft-v1` 🟡 → ZhenfaTrap 类目（4 类阵预埋件配方）—— v2 P0 必须先用此底盘
- `plan-meridian-severed-v1` 🆕 active → 阵法流派依赖经脉清单（任督二脉 + KI 足少阴肾经）+ SEVERED 阵法失效路径
- `plan-tribulation-v2` 🆕 active → 聚灵阵天道注视累积 + 欺天阵被识破触发绝壁劫（天地排异三相）
- `plan-narrative-political-v1` ✅ active → 欺天阵江湖传闻
- `plan-social-v1` ✅ → 灵龛归属（护龛阵需要灵龛主人身份认证）
- `plan-skill-v1` ✅ + `plan-input-binding-v1` ✅ + `plan-HUD-v1` ✅
- `plan-multi-style-v1` ✅ → 缜密色 PracticeLog
- `plan-anqi-v1` ✅ → 共享 `CarrierImprint` 字段（v1 双 plan 已对齐）
- `plan-cultivation-canonical-align-v1` ✅ → Realm + 经脉拓扑

**反向被依赖**：

- `plan-style-balance-v1` 🆕 → 5 阵的 W/ρ 数值进矩阵（阵法 ρ=0.40 / 专克毒蛊 W=0.6 因聚灵阵反污染 / 失效 vs 涡流 W=0.3 因紊流场冲散阵眼）
- `plan-tribulation-v2` 🆕 active → 欺天阵化虚级触发天道注视（同 zhenmai-v2 / baomai-v3 / anqi-v2 化虚级同列）
- `plan-multi-life-v1` ⏳ → 跨周目阵法持久化处理（化虚阵法师跨周目还能维护吗？）
- `plan-yidao-v1` 🆕 placeholder → 护龛阵 + 医者结合（医者诊所被护龛阵保护路径）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation` / `cultivation::MeridianSystem` / `qi_physics::field::*`（逆逸散、密度场）/ `qi_physics::ledger::QiTransfer`（阵法真元封存守恒律）/ `combat::carrier::CarrierImprint`（v1 已实装，v2 扩 `array_kind` 字段）/ `inventory`（载体素材 + 阵旗）/ `craft::CraftRegistry`（4 类阵预埋件配方）/ `social::ShrineOwnership`（灵龛归属）/ `tsy::DimensionRegistry`（子位面）/ `agent::tiandao::TribulationWeights`（欺天阵广播目标）/ `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm`
- **出料**：4 阵 `ZhenfaArrayKind` enum 注册到 ZhenfaRegistry（v1 已建立，v2 扩 4 个变体）/ `WardArrayDeployEvent` 🆕（护龛布阵）/ `LingArrayDeployEvent` 🆕（聚灵布阵 + 天道注视累积）/ `DeceiveHeavenEvent` 🆕（欺天阵 zone 快照伪造）/ `IllusionArrayDeployEvent` 🆕（幻阵覆盖现有阵法）/ `ArrayDecayEvent` 🆕（4 类阵朽坏统一事件）/ `ArrayBreakthroughEvent` 🆕（被破解事件）
- **共享类型**：`ArrayImprint` component（继承 v1 `CarrierImprint`，扩 `dimension_target: Option<DimensionId>` + `tribulation_broadcast: bool` 字段）/ `ArrayMastery` component（mastery 0-100 + 5 阵各自的 lv 字段）
- **跨仓库契约**：
  - server: `combat::zhenfa_v2::*` 主实装（v1 诡雷 + 警戒场迁入 + v2 新 5 阵）/ `schema::zhenfa_v2`
  - agent: `tiandao::zhenfa_v2_runtime`（4 阵 narration + 欺天阵被识破反噬叙事 + 聚灵阵天道注视提示）
  - client: 4 阵动画（布阵 / 触发 / 朽坏 / 破阵）+ 4 粒子 + 4 音效 recipe + 5 HUD 组件（阵图布局 / 阵眼 marker / 朽坏倒计时 / 天道注视进度 / 经脉依赖灰显）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：阵法真元逆逸散走 `qi_physics::field::inverse_diffusion` 🆕 / 聚灵阵真元密度增益走 `qi_physics::field::density_amplifier` 🆕 / 欺天阵天道感应扰动走 `qi_physics::field::tiandao_signal_distort` 🆕（patch P3 全加） / 朽坏速率走 v1 已实装 `qi_physics::container::abrasion_loss`（共享 anqi-v2 magnitude 不同 tax_rate）/ **禁止 plan 内自己写阵法 / 朽坏 / 子位面投射 / 天道扰动公式**

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

- [ ] **4 阵完整范围（v2）**：
  ```
  ① 护龛阵     | 灵龛防御     | 反弹/阻挡场 + 信誉度认证      | worldview §五.3
  ② 聚灵阵     | 真元密度增益 | 真元逆逸散场 + 天道注视风险   | worldview §八:602
  ③ 欺天阵     | 信息战       | zone 快照伪造欺骗天道 agent   | worldview §八:614
  ④ 幻阵       | 隐蔽变体     | 覆盖现有阵法的可见性谱          | 4 大类附属
  ```
  ~~⑤ 跨位面阵~~ — 砍掉（2026-05-10）：子位面实例化机制尚未实装（plan-tsy-dimension-v1 只有单 Layer），等子位面多实例后再立 zhenfa-v3 或独立 plan

- [ ] **已延期：化虚级跨位面阵物理推导（worldview §三:187 + §十三）**：子位面多实例尚未落地，相关机制统一留到 zhenfa-v3，不计入 v2 验收。
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

- [ ] **熟练度生长二维划分（v2 通用机制）**：
  - **境界**：决定阵威上限（伤害 / 持续时长 / 信号广播范围 / 真元密度增益）
  - **熟练度 mastery (0-100)**：决定布阵速度（cast time）/ 朽坏延缓（持续 ×1→×3）/ 逆逸散效率
  - 4 阵各自有 `mastery: u8` 字段，cast 一次 +0.3 / 阵被触发 +1.0
  - 数值表见 §1 各阵规格

- [ ] **专属物理边界 = 长期场（与 anqi 单点 / zhenmai 瞬时痕迹 / dugu 30min 残留 / tuike 蜕落物 30min 区分）**：
  - 阵法朽坏期：30min - 数小时（载体材质决定，跟 anqi-v2 6 档载体共享 carrier_grade）

---

## §1 四阵完整规格

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

**功能**：在阵法范围内制造真元密度场扰动，让天道 agent 的环境感知**采不到阵内实体的真实数据**（worldview §八:614-618 气运劫持）。不是"向天道广播假信号"——天道 agent 消费的是 Redis `bong:world_state` 里的 zone 快照，欺天阵的物理效果是**让 server 在该 zone 快照中抹掉 / 替换阵内实体的 qi_color / realm / tribulation_weight 等字段**，agent 侧无需任何特殊处理，看到的就是假数据，自然不会把该区域列为劫期目标。

**境界要求**：固元+ 威力门坎（醒灵 / 引气 / 凝脉 cast 不出来，因为真元密度场扰动强度不足以覆盖天道采样精度）

**真元消耗**：极高 80-100% qi_max + 罕见材料（5 个化虚级真元封存的载体）

**冷却**：300s → 120s（mastery 0→100，每次施展冷却 + 业力累积）

**效果**：
- 60s 内 server 在 `bong:world_state` zone 快照中**替换阵内实体的 tribulation_weight / qi 相关字段为伪值**：
  - 自身 tribulation_weight 快照值 × 0.5（天道 agent 看到的劫期权重降一半）
  - 选定目标 tribulation_weight 快照值 × 1.5（嫁祸——天道 agent 看到该目标劫期权重升 50%）
  - 阵内所有实体的 qi_current / qi_max 快照值替换为低值（让天道认为这片区域灵气稀薄、不值得注视）
- 应用场景：
  - PvP 渡劫干扰：把对方推上劫期巅峰（天道 agent 自然优先关注高权重目标）
  - 逃避自身劫期：60s 窗口内天道 agent 采样到的自身权重被压低
  - 嫁祸：让无关目标承受天道注视
- **被识破机制**：天道 agent 有交叉校验（多 zone 真元总量守恒断言）——如果阵法范围内上报的 qi 总量与相邻 zone 流入流出账本不符，agent 侧会标记异常。server 每 tick 0.5%（化虚 0.2%）概率触发"守恒校验失败" → 欺天阵被穿透 → 反噬 ×3（真实 tribulation_weight 暴露 + 业力反弹）

**载体**：1 个化虚级阵眼（上古阵眼方块）+ 5 个上古残骨封真元 + 1 把化虚阵旗

**朽坏期**：1h（一次施展即朽坏）

**经脉依赖**：任督 + KI + 心经（高级真元广播），SEVERED 任一 → ③ 失效

**化虚级专属（v2 P0 默认即化虚级）**：识破概率 0.5% → 0.2%/tick + 反噬倍率不变（×3）—— 化虚阵法师玩"长期欺天"赌识破

**反 MMO 红线**：见 §0 第 4 条 — 长期统计劫期不变小，只是延后 + 转嫁，不是免疫。天道 agent 仍然正常运作——它只是被喂了假数据，不需要 agent 侧写特殊逻辑

**触发天道注视**：每次施展 +tribulation_weight × 1.5（worldview §八:614 物理化身）—— 跟其他流派化虚级同列

**测试饱和**：欺天阵 30 单测（zone 快照字段替换 / 守恒校验失败触发 / 反噬 / 嫁祸权重转嫁 / 业力累积 / 化虚级数值差异 / 天道 agent 消费伪快照后行为正常）

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

~~### ⑤「跨位面阵」— 砍掉（2026-05-10），留 zhenfa-v3~~

---

## §2 经脉依赖（接 plan-meridian-severed-v1）

阵法流派依赖经脉清单：

| 经脉 | 阵依赖 | SEVERED 来源 | SEVERED 后果 |
|---|---|---|---|
| 任脉 `Ren` | ①②③ 阵眼维持 | OverloadTear（聚灵阵超浓度过载）/ TribulationFail / DuguDistortion | 阵法流派几乎全废 |
| 督脉 `Du` | ①②③ 真元广播 | OverloadTear / TribulationFail（化虚雷劫炸）/ DuguDistortion / VoluntarySever | 信号广播能力丧失 |
| 足少阴肾经 `Kidney` | ②④ 真元逆逸散调控 | DuguDistortion（脏真元蚀脉）/ CombatWound | 聚灵 / 幻阵失效 |
| 手少阴心经 `Heart` | ③ 高级真元广播 | OverloadTear（欺天阵被识破反噬）/ TribulationFail | 欺天阵失效 |

**v2 实装**：在 plan-meridian-severed-v1 7 流派经脉表追加阵法条目；P3 阶段交付。

---

## §3 craft-v1 配方接入（4 阵预埋件 + 阵旗）

接 `plan-craft-v1` 🟡，ZhenfaTrap 类目添加 4 阵专属预埋件 + 2 档阵旗：

| 配方 ID | 类目 | 输出 | 输入 | 时间 | 解锁渠道 |
|---|---|---|---|---|---|
| `zhenfa.array.ward` | ZhenfaTrap | 护龛阵预埋件 | 上古残土碎 ×5 + 阵眼方块 ×1 | 15 min | 残卷 |
| `zhenfa.array.lingju` | ZhenfaTrap | 聚灵阵预埋件 | 上古残土碎 ×9 + 阵眼方块 ×3 + 灵泉水 ×3 | 30 min | 师承 |
| `zhenfa.array.deceive` | ZhenfaTrap | 欺天阵预埋件 | 上古残骨 ×5 + 阵眼方块 ×1 + 真元封存 80% qi_max | 60 min | 顿悟（固元+） |
| `zhenfa.array.illusion` | ZhenfaTrap | 幻阵符 ×3 | 缜密色染料 ×3 + 灵木 ×2 | 5 min | 师承 |
| `zhenfa.flag.basic` | Tool | 基础阵旗 | 灵木 ×2 + 兽皮 ×1 + 缜密色染料 ×1 | 8 min | 默认 |
| `zhenfa.flag.deceive` | Tool | 欺天阵旗 | 上古残骨碎 ×3 + 真元封存 50% qi_max | 30 min | 顿悟（固元+） |

**v2 实装**：P0 阶段把这些配方写进 craft-v1 配方表（不是 plan 内自己实装 craft 系统）。

---

## §4 客户端动画 / VFX / SFX

| 阵 | 布阵动画 | 触发 / 效果粒子 | 朽坏粒子 | 音效 |
|---|---|---|---|---|
| ① 护龛阵 | 持旗围灵龛走 5 步（缜密色光纹画圆） | 反弹光晕 + 阻挡推力波 | 缜密色灰飞 | 咏唱 + 反弹叮 + 阻挡嗡 |
| ② 聚灵阵 | 持旗布 3-9 阵眼（每点缜密色立柱） | 真元密度场涟漪 + 灵气可视化 | 阵柱倒塌 | 咏唱 + 灵气嗡（持续） |
| ③ 欺天阵 | 持化虚阵旗对天画符 | 假劫气云团升起（向天道方向）+ 识破时反噬雷光 | 阵眼熔毁 | 咏唱 + 假劫云嗡 + 反噬雷鸣（识破） |
| ④ 幻阵 | 持旗甩出符纸覆盖现有阵法 | 缜密色雾化覆盖 + 神识扫到时显形涟漪 | 雾散 | 符纸碎裂 + 显形咔嚓 |
| ⑤ 跨位面阵 | 已延期至 zhenfa-v3 | 已延期至 zhenfa-v3 | 已延期至 zhenfa-v3 | 已延期至 zhenfa-v3 |

HUD 组件（plan-HUD-v1 接入）：

- **阵图布局 UI**（持旗 Shift+使用键）：4 阵图标 + 阵眼布置预览 + 阵参数配置（载体材质 / 阵威 / 朽坏期）；跨位面阵图标延期至 zhenfa-v3
- **阵眼 marker**：地图上显示已布阵眼位置 + 朽坏倒计时
- **天道注视进度**：聚灵阵 / 欺天阵布阵时显示当前 tribulation_weight（圈环）；跨位面阵显示延期至 zhenfa-v3
- **假劫期广播指示**：欺天阵激活时显示 "60s 假劫期生效中" + 识破概率实时显示
- **跨位面通道**：已延期至 zhenfa-v3，v2 不实现活跃通道列表。
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

### P3 — 经脉依赖 + 熟练度生长（4 周）

- [ ] 接 `plan-meridian-severed-v1` 7 流派经脉表追加阵法条目
- [ ] 4 阵 SEVERED 失效路径
- [ ] 4 阵 mastery 字段（0-100）+ cast/触发加 mastery 公式
- [ ] mastery 生长效果（cast time / 朽坏延缓 / 逆逸散效率）
- [ ] 测试：经脉依赖 25 单测 + mastery 30 单测

### P4 — 客户端 4 动画 / 4 粒子 / 4 音效 / 5 HUD（3 周）

- [ ] 4 阵动画（布阵 / 触发 / 朽坏 / 破阵）
- [ ] 4 粒子 + 4 音效 recipe
- [ ] 5 HUD 组件（阵图布局 / 阵眼 marker / 天道注视 / 假劫期 / 经脉依赖灰显）
- [ ] 测试：客户端 4 阵视觉回归 + HUD 集成测试

### P5 — v2 收口（饱和测试 + agent narration + e2e 联调）（2-3 周）

- [ ] agent `tiandao::zhenfa_v2_runtime`（4 阵 narration 全量 + 欺天阵识破触发绝壁劫叙事）
- [ ] 欺天阵被识破 → emit `JueBiTriggerEvent`（plan-tribulation-v2 接入）
- [ ] e2e 联调：client → server cast → 阵法布置 → 触发 / 朽坏 → client 渲染（每阵独立 e2e）
- [ ] 饱和测试 audit：4 阵每阵 ≥20 单测，总单测 ≥120
- [ ] 与 plan-style-balance-v1 对接：阵法 W/ρ 矩阵填表（vs 7 流派 7 个 W 数值）
- [ ] Finish Evidence + 迁入 docs/finished_plans/

---

## §6 已知风险 / open 问题

- [ ] **Q2** 欺天阵识破概率（0.5% / 0.2%）：是否需要根据天道权重动态调整 → P1 拍板
- [ ] **Q3** 聚灵阵天道注视阈值（zone_density × 1.5 > 6.0）：v1 留待 Q14 拍板，v2 需根据 lingtian 系统实测调参 → P0 拍板
- [ ] **Q4** 多个聚灵阵叠加：阵眼位置近 → 浓度叠加是否触发更高天道注视 → P0 拍板
- [ ] **Q5** 幻阵 vs 神识扫描：是否给被扫描者主动反馈"被扫描"信号 → P2 拍板
- [ ] **Q7** 欺天阵嫁祸 PvP：被嫁祸玩家如何感知 + 反制路径 → P1 拍板（与 plan-narrative-political-v1 联动）

---

## §7 进度日志

- 2026-05-06：骨架创建。承接 plan-zhenfa-v1 ✅ finished（commit b82b02a0；P0/P1 诡雷 + 警戒场已实装）。早期骨架曾包含化虚跨位面阵；2026-05-10 已确认子位面多实例尚未落地，该项延期至 zhenfa-v3。v2 范围收敛为 4 大类剩余 3 类（护龛 / 聚灵 / 欺天）+ 幻阵 + 5 经脉依赖 + 熟练度生长 + 反 MMO 物理推导。欺天阵走 worldview §八:614-618 气运劫持（**不是免疫**，是真元场扰动天道感应器，被识破反噬 ×3）。聚灵阵天道阈值落地（v1 Q14）。坐标派 / 信息派定调，与暗器单点 / 截脉接触面 / 毒蛊渗透 / 替尸钱包 / 体修肉体 / 涡流环境改造区分。

## Finish Evidence

### 落地清单

- **server 阵法核心**：在 `server/src/zhenfa/mod.rs` 复用 v1 `ZhenfaRegistry` / 阵眼生命周期，扩展 `ZhenfaKind::{ShrineWard,Lingju,DeceiveHeaven,Illusion}`，新增 `ArrayImprint`、`ArrayMastery`、四阵 deploy 事件、统一 decay / breakthrough 事件与欺天暴露专用事件。
- **护龛 / 聚灵 / 欺天 / 幻阵**：护龛阵按 owner + `Relationships` 盟友 + `Renown.fame >= 80` 放行，未授权目标进入范围会被阻挡和烧伤；聚灵阵写入密度倍率与天道注视权重 profile；欺天阵固元+ gate、定 tick 识破后触发 `JueBiTriggerEvent::ZhenfaDeceptionExposed`；幻阵使用独立 `reveal_threshold` 契约，不复用 `radius`。
- **经脉与 mastery**：布阵前检查 `MeridianSeveredPermanent`，四阵经脉依赖分别接任督 / Kidney / Heart；`ArrayMastery` 按 cast +0.3、触发 +1.0 增长，并参与 cast time / 持续时间 profile。
- **qi_physics**：`server/src/qi_physics/field.rs` 新增 `inverse_diffusion`、`density_amplifier`、`tiandao_signal_distort`，阵法模块只消费底盘算子，不在 plan 内另造物理公式。
- **craft 接入**：`server/src/craft/mod.rs` 注册 `register_zhenfa_v2_recipes()`，落地 4 个阵法预埋件和 2 档阵旗配方，并保持 `zhenfa.*` 命名空间。
- **跨栈契约 / IPC**：新增 `server/src/schema/zhenfa_v2.rs`、`server/src/network/zhenfa_v2_event_bridge.rs`、`CH_ZHENFA_V2_EVENT`、`RedisOutbound::ZhenfaV2Event`；agent schema 新增 `ZhenfaV2EventV1` 与 generated JSON；client request schema / Java enum 同步 4 个新 kind。
- **agent narration**：`agent/packages/tiandao/src/zhenfa-v2-runtime.ts` 订阅 `bong:zhenfa/v2_event`，覆盖 deploy / decay / breakthrough / `deceive_heaven_exposed` 叙事，并在 `main.ts` 启动 runtime。
- **client 协议**：`ClientRequestProtocol.ZhenfaKind` 支持 `shrine_ward`、`lingju`、`deceive_heaven`、`illusion`，并补 `deceive_heaven` 编码回归；本 PR 不宣称完成计划中 P4 的专用动画 / 粒子 / 音效 / HUD 资产。

### 测试结果

- `cd server && cargo fmt --check`：通过。
- `cd server && cargo check`：通过。
- `cd server && CARGO_BUILD_JOBS=1 cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0" cargo test zhenfa`：通过，28 passed / 0 failed / 3635 filtered out。
- `cd server && CARGO_BUILD_JOBS=1 RUSTFLAGS="-C debuginfo=0" cargo test`：通过，3663 passed / 0 failed。
- `cd agent && npm run generate -w @bong/schema`：通过，326 schemas exported。
- `cd agent && npm run build`：通过。
- `cd agent && npm test -w @bong/schema`：通过，15 files / 353 tests。
- `cd agent && npm test -w @bong/tiandao`：通过，47 files / 329 tests。
- `cd client && JAVA_HOME="<java-17-home>" PATH="$JAVA_HOME/bin:$PATH" ./gradlew test build`：通过，BUILD SUCCESSFUL（本地使用 Corretto 17.0.18）。
- `git diff --check`：通过。

### 备注 / 后续

- 普通 `CARGO_BUILD_JOBS=1 cargo test zhenfa` 在本机 debug test binary 链接阶段曾被 `SIGKILL`；使用 `RUSTFLAGS="-C debuginfo=0"` 后定向与全量 server 测试均通过，判定为本地链接内存压力而非源码失败。
- plan 文本早期写过"跨位面阵"和 P4 专用视觉/HUD 全量交付；2026-05-10 范围已收敛为 v2 runtime / contract / physics / craft / narration / protocol 闭环，跨位面阵留 zhenfa-v3，专用视觉资产留后续 plan。
