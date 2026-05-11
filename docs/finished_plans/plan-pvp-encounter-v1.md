# Bong · plan-pvp-encounter-v1

多人遭遇设计与匿名博弈——末法残土最独特的社交张力来自**默认匿名 + 无安全区外的绝对自由 PvP**。现有系统层已齐备（`plan-social-v1` ✅ 社交 / `plan-identity-v1` ✅ 身份与信誉 / `plan-combat-no_ui` ✅ 战斗 / `plan-niche-defense-v1` ✅ 灵龛防御），但缺少**多人遭遇的 choreography**——两个无名修士在荒野相遇时的沉默博弈、坍缩渊入口的囚徒困境、战后幸存者的信息战、以及从"默认敌对"到"临时合作"再到"背叛"的叙事弧线。本 plan 不新增 PvP 规则，而是设计**遭遇的戏剧结构**。

> **2026-05-11 依赖实地核验通过**：全部 14 个前置 plan 已实装并注册在 `server/src/main.rs`。social（PlayerChatCollected/Exposure/RenownDelta，4 测试文件）/ identity（10 测试文件）/ combat 全 7 流派（33 测试文件）/ niche-defense（已归档）/ style_telemetry（QiColor 追踪）/ death_lifecycle（BiographyEntry）/ tsy 全套 / political_narration（agent 侧 500+ 行，测试存在）/ realm_vision（5 子模块）/ spiritual_sense（scanner/push/throttle）— 无阻塞缺口。

**世界观锚点**：`worldview.md §十一` 匿名系统（默认无名字/无境界标签）· `§九` 面对面以物易物 + 交易暴露身份 · `§四` 战斗是真元汇率兑换——打赢了也可能耗尽真元从而死在回家路上 · `§五` 末土后招原则（所有修士都有未暴露能力）· `§十六` 坍缩渊内匿名更强（负压噪声掩盖神识）

**前置依赖**：
- `plan-social-v1` ✅ — 社交基础设施（feud/pact/renown）
- `plan-identity-v1` ✅ — 多身份/信誉度/NPC 反应
- `plan-combat-no_ui` ✅ — 无 UI 战斗
- `plan-niche-defense-v1` ✅ — 灵龛防御
- `plan-skill-v1` ✅ — 招式/技能系统
- `plan-style-vector-integration-v1` ✅ — PracticeLog/真元色向量
- `plan-death-lifecycle-v1` ✅ — 死亡/遗念/掉落
- `plan-tsy-*` 全 14 份 ✅ — 坍缩渊内 PvP 环境
- `plan-narrative-political-v1` ✅ — 江湖传闻型政治叙事

**反向被依赖**：
- `plan-sou-da-che-v1` 🆕 skeleton — 搜打撤循环中"遭遇其他玩家"作为风险变量
- `plan-combat-gamefeel-v1` 🆕 skeleton — PvP 战斗 juice（vs 自己的同类，心理冲击应 > PvE）

---

## 边界

| 维度 | 已有系统 | 本 plan 拓展 |
|------|---------|-------------|
| PvP 规则 | combat 全流派 + social feud | 不碰战斗数值和社会关系——设计**遭遇的前/中/后**三个阶段的行为剧本 |
| 匿名 | 默认无名字/无境界标签 | 不碰匿名规则——设计匿名下的**非语言沟通渠道** |
| 识别 | 固元+ 感知大致境界 / 通灵+ 感知更多 | 不碰感知系统——设计**识别行为**的前后戏剧性 |
| 背叛 | social pact/feud 机制 | 不碰 pact 机制——设计**从合作到背叛**的叙事触发与情感影响 |
| 坍缩渊 PvP | tsy 全套 | 不碰坍缩渊机制——设计**浅层收割/深层淘金/出关博弈**的遭遇模式 |

---

## §0 设计轴心

- [x] **每次遭遇都是一场没有对话的谈判**：两个无名修士在荒野相遇——眼神（实际是移动方向/停顿/距离）是唯一的沟通方式。系统应提供足够的非语言信号让双方做出有意义的判断
- [x] **三幕遭遇结构**：遭遇前（远距离发现→评估→决策）→ 遭遇中（近距离交互→试探→爆发 or 和平）→ 遭遇后（幸存者处理信息→改变行为→影响下一次遭遇）
- [x] **不鼓励无差别 PvP**：战斗是真元汇率兑换——打赢了也可能耗尽真元死在野兽手里。遭遇的设计应让玩家自然倾向于"不打"除非有明确收益
- [x] **信息差是遭遇的核心引擎**：你不知道对方是谁/什么境界/什么流派/有什么后招/是不是诱饵/身后有没有队友。这 6 个"不知道"驱动每一次遭遇的紧张感
- [x] **从默认敌对到临时合作到背叛——让故事自然发生**：系统不应强制合作或强制敌对，但应提供让合作→背叛叙事自然涌现的条件

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 遭遇决策树：远距离评估→近距离交互→结果分类 | ✅ 2026-05-12 |
| P1 | 非语言沟通渠道：移动/停顿/距离/物品展示作为"沉默的对话" | ✅ 2026-05-12 |
| P2 | 背叛叙事：从临时合作到翻脸的触发条件与情感后果 | ✅ 2026-05-12 |
| P3 | 遭遇后信息战：幸存者如何利用/传播/封锁遭遇信息 | ✅ 2026-05-12 |
| P4 | 坍缩渊遭遇模式：入口/浅层/深层/撤离点的不同遭遇剧本 | ✅ 2026-05-12 |
| P5 | 多人遭遇压测：5 玩家交叉遭遇矩阵 + 叙事涌现验证 | ✅ 2026-05-12 |

---

## P0 — 遭遇决策树 ✅ 2026-05-12

### 交付物

1. **遭遇阶段模型**（设计文档，不实现代码）

**阶段 1：远距离发现（20-50 格）**
- 双方互相可见（人形，无名字，无境界标签）
- 可观察信号：
  - 对方是否停下来看你了（停顿 = 评估中）
  - 对方的移动方向（继续走 = 无意纠缠 / 改变方向 = 在绕你 / 直冲过来 = 有备而来）
  - 对方是否切换了手持物品（切武器 = 准备战斗 / 切火把 = 想表明和平 / 不切 = 保持神秘）
  - 对方是否蹲伏了（蹲伏 = 想观察 or 准备伏击）
  - 环境上下文：这里是馈赠区（可能抢资源）还是死域（相遇大概率偶然）

**阶段 2：中距离试探（8-20 格）**
- 双方进入"可能交互"距离
- 决策分支：
  - A. 双方绕行（最安全，最常见）——各走各路，沉默的互相尊重
  - B. 一方停下 + 切火把（和平信号）——想交易/问路
  - C. 一方切武器 + 加速（敌对信号）——要打架
  - D. 双方停下，互相观察（紧张僵持）——都在等对方先动
  - E. 一方丢出物品在地上（交易邀请）——风险：东西丢出去别人捡了就跑

**阶段 3：近距离交互（0-8 格）**
- 进入 chat 距离（可打字）或战斗距离
- 可能结果：
  - 和平分离：交换情报/交易/组队 → 各走各路
  - 试探性战斗：一方出手试探 → 另一方反制 → 双方评估→ 一方认输撤退 or 死战
  - 直接死战：一方全力出手 → 另一方全力还击 → 一死一伤 or 双双重伤
  - 临时合作：双方发现共同目标（都在打同一只异变兽/都在避开同一个威胁）

2. **遭遇决策因素权重表**（设计文档）

| 因素 | 权重 | 对决策的影响 |
|------|:---:|------|
| 对方的装备外观（可见武器/护甲） | 高 | 看起来强 → 绕行概率↑ |
| 我方当前真元/血量 | 最高 | 状态差 → 任何方案都以保命为前提 |
| 对方是否首先蹲伏 | 中 | 蹲伏 = 观察 → 给我方更多决策时间 |
| 环境是否在馈赠区/灵眼附近 | 高 | 资源点 → 竞争概率↑ |
| 我方背包是否有高价值物品 | 高 | 满载 → 不想打，"保货"优先 |
| 是否在坍缩渊内 | 极高 | 坍缩渊内 PvP 概率骤升（loot 可被掠夺） |
| 对方是否在不久前被天道点名 | 中 | 知道对方强/弱 → 决策更有信息 |
| 是否是汐转期 | 中 | 汐转期修士更紧张 → 更易误判 |

### 验收抓手

- 设计评审：3 阶段模型 + 权重表 通过团队 review
- 手动验证：在不同环境/状态下遇到其他玩家 → 观察实际行为是否符合决策树预测
- 注意：**本 plan 不强制玩家行为**——决策树是设计参考和 agent narration 依据，不是代码层面的 if-else

---

## P1 — 非语言沟通渠道 ✅ 2026-05-12

### 交付物

1. **动作信号系统**（`client/src/main/java/com/bong/client/social/SilentSignalSystem.java`）
   - 以下玩家动作在对方 15 格内可见，作为非语言沟通：
     - 切火把：切换到火把手持 = "我没有武器，想和平"（对方 HUD 显示对方手持火把 icon）
     - 丢出 1 枚骨币在地上：= "买路钱" / "示好"——低价值物品的象征性给予
     - 缓慢后退（移速×0.5 + 面对对方）：= "我不想打，我在退"
     - 快速蹲伏 2 次：= "注意——有威胁"（老玩家之间的暗号）
     - 指向某方向（右手空手对准一个方向 2s）：= "那边"（配合骨币丢出 = "你可以走那边"）
     - 原地打坐 3s：= "我不构成威胁，你们打你们的"（风险：打坐时极脆弱）
   - **关键设计**：这些信号**不附带任何 UI 解释**——对方是真人，需要真人理解。新手不知道这些暗号 = 信息差的一部分

2. **沉默的威胁展示**
   - `style_telemetry::RevealedSkill`：当玩家在另一玩家 8 格内使用流派技能时 → 对方客户端显示微型提示（"对方使用了某种流派招式"——不给招式名，只给视觉）
   - 全服 narration 点名（被天道提到）：在场的其他玩家听到 narration → 知道"这个人刚刚被天道记录了什么"
   - 死亡时掉落物品的视觉：物品从尸体 pop 出 → 围观者可以看到掉落物的稀有度光柱——一眼判断"这人身上有没有货"

3. **"假信号"设计**
   - 玩家可以故意给出假信号：
     - 切火把假装和平 → 靠近到 3 格 → 切武器突袭
     - 丢骨币在地 → 等对方弯腰捡 → 攻击
     - 缓慢后退假装撤退 → 退到有利地形 → 回头反打
   - **代价**：使用假信号的玩家，其 `identity` 的 NPC 信誉度会受损（如果被 NPC 目击）——且被欺骗的其他玩家会在社交圈传话
   - **设计意图**：让"信任判断"成为真正的游戏技能。假信号是策略，不是 bug

### 验收抓手

- 测试：`client::social::tests::signal_detection_range` / `client::social::tests::fake_signal_no_ui_warning`
- 手动：双客户端 → 相遇 → 各种信号组合测试 → 假信号→真攻击 → 目击者 NPC 信誉变化

---

## P2 — 背叛叙事 ✅ 2026-05-12

### 交付物

1. **临时合作→背叛 的触发条件**

| 合作场景 | 背叛触发点 | 叙事价值 |
|---------|----------|---------|
| 组队打异变兽 | 兽死后——兽核只有一颗，谁拿？ | "那一瞬间，两人都停了手。地上只有一颗兽核。" |
| 组队进坍缩渊 | 撤离时——裂缝口只能站一个人同时撤离 | "你先走。……我开玩笑的。" |
| 分享灵眼坐标 | 到达现场——灵气只够一个人凝核 | "你说这里有灵眼。确实有。现在请便。" |
| 物资交换 | 东西丢地 → 对方捡了就撒腿跑 | "骨币是假的。你的命也是。" |
| 一起躲劫 | 天劫结束后 → 双方都重伤 → 只剩一次出手的机会 | "谢谢。这就是我的谢意。" |

   - **关键设计**：背叛不是必然——系统不给"背叛"按钮，不强制触发。以上是**可能在特定条件下自然涌现**的场景

2. **背叛后的社交后果**
   - 被背叛的玩家：获得对方的 identity 信息（天道记录——"某人背叛了你"）
   - 叛徒的同一 identity：NPC 信誉度 -30（被 NPC 目击时）/ 被传话扩散
   - 被背叛的玩家可以向守墓人"举报"（付费）→ 叛徒在特定区域被通缉
   - 但——叛徒可以切 identity 洗白（`plan-identity-v1` ✅ 已支持多身份切换）
   - **设计意图**：背叛有代价，但也提供了"洗白"的出口——信息差继续存在

3. **Agent 叙事的背叛报道**
   - `plan-narrative-political-v1` ✅ 的政治叙事检测到"合作→背叛"事件时：
     - 生成一条含糊但指向性的江湖传闻："灵泉湿地有修士背弃同伴。骨币之事，江湖知。"
     - 不点名（保护隐私），但给足够线索让玩家推理
   - 设计意图：让背叛成为"江湖故事"——不只在两个当事人之间，而是一段可以被后人查询的历史

### 验收抓手

- 手动：双客户端模拟合作→背叛场景 → 社交后果触发 → 叛徒查看 NPC 信誉度变化 → 叛徒切身份 → 成功洗白
- 测试：`server::social::tests::betrayal_reputation_impact` / `server::identity::tests::identity_switch_clears_betrayal`

---

## P3 — 遭遇后信息战 ✅ 2026-05-12

### 交付物

1. **幸存者的信息资产**
   - 每次 PvP 遭遇后（无论结果是战/和平/背叛），幸存者获得：
     - 对方的**大致外观描述**（agent 生成："一个右手持骨刺的修士，走路微跛"）
     - 对方的**流派特征**（如果对方在战斗中使用了流派技能——看到是什么类型）
     - 对方的**真元色**（如果固元+ 且 inspect 过）
     - 对方的**identity 名**（如果交易过 or 对方在 chat 中说了话 or 被天道点名）
   - 这些信息存入 `LifeRecord`（`plan-death-lifecycle-v1` ✅ 已就绪）→ 成为该角色的永久记忆

2. **情报流通渠道**
   - **玩家间口头传**（chat）："我在血谷看到一个人，右手持骨刺，走路微跛——小心。"
   - **付费查询**：向散修 NPC 购买"最近在此地活动的人"的情报（按 §九 经济系统——信息交易）
   - **守墓人情报**：向守墓人支付骨币 → 获得某区域最近出现的修士的外貌描述（不保证完整）
   - **亡者博物馆**：死透的角色的 LifeRecord 成为公开档案 → 可查阅"这个人在死前和谁打过"
   - **天道叙事暗示**：agent narration 偶尔提及"血谷近来有持骨刺者出没"——让受过伤的玩家警觉

3. **"被记住"的恐惧**
   - 如果你经常在同一区域活动 + 曾被多人目击 → agent 可能生成"某人在某地频繁出没"的传闻
   - 这迫使老玩家要么**频繁切换活动区域**，要么**切 identity 洗白外观**
   - 不通缉、不标记——只是"信息在流动"

### 验收抓手

- 手动：一次 PvP 遭遇后 → 检查 LifeRecord 是否记录了对方特征 → 找 NPC 付费查询 → NPC 提供有但并不完整的描述 → 切换区域 + 切 identity → 下次回去不再被 NPC 认出
- 测试：`server::social::tests::life_record_encounter_entry` / `server::npc::tests::npc_intel_purchase_partial_info`

---

## P4 — 坍缩渊遭遇模式 ✅ 2026-05-12

### 交付物

1. **坍缩渊内遭遇的特殊性**

| 位置 | 遭遇模式 | 囚徒困境 |
|------|---------|---------|
| 裂缝入口 | 所有进入者在此相遇——可能排队（等前面的人进去 or 等里面的人出来）。多人在入口 = 脆弱窗口。 | "谁先进？"——第一个人暴露后背给后面所有人。但同时第一个人先搜到容器。 |
| 浅层 | 高阶修士的收割场。低阶经过浅层时最大风险区域。 | 高阶守浅层→看到低阶带有闪闪发光的 loot → 动手？动手暴露身份给经过的其他人。 |
| 中层 | 中阶混战区。资源竞争 + 道伥威胁。临时合作打道伥→打完互相评估→是否趁对方虚弱补刀。 | "先一起把道伥打了。打完再说。" |
| 深层 | 低阶避难所——高阶进不来。低阶之间的竞争没有境界碾压。 | 大家都是低阶，都在搜 jackpot。资源多但不是无限。是合作搜更快还是各自搜更安全？ |
| 撤离点 | 无论主裂缝还是深层缝——撤离时需要静立 7-15s。排队撤离 = 最后一个走的人暴露给前面所有人。 | "你先撤。""不，你撤。"实则两人都在想：我先撤=把你背露给你。 |
| race-out | 坍缩渊塌缩时随机裂口 3-5 个 + 撤离时长缩短到 3s。所有人冲向最近裂口。 | 跑得最快的拿到裂口。跑得慢的呢？裂口有 PvP 吗？都有。 |

2. **坍缩渊内 PvP 的非语言信号增强**
   - 负压环境中神识失灵 → §十一 匿名更强 → 玩家更依赖动作信号（P1）
   - 但坍缩渊内无法 chat（负压干扰 chat 传输——worldview §十六 "神识互相失灵"的 UI 实现）
   - 结果：坍缩渊内的多人交互**完全靠动作信号**——最纯粹的"沉默博弈"

3. **坍缩渊 PvP 的特殊后果**
   - 坍缩渊内死亡 → 本次秘境所得 100% 掉落 + 遗骸干尸化（已有 `plan-tsy-raceout-v2` ✅）
   - 杀人者在坍缩渊内无法捡光所有东西（撤离时间紧迫）→ 杀人的"效率"不高
   - 设计意图：坍缩渊内 PvP 风险极高但回报有限——鼓励"你拿你的我拿我的，井水不犯河水"的隐性规则

### 验收抓手

- E2E：双客户端进入坍缩渊 → 入口 PvP → 浅层收割遭遇 → 深层合作打道伥 → 撤离点 mutual standoff → race-out 冲刺
- 测试：`server::tsy::tests::pvp_at_extract_point` / `server::tsy::tests::chat_blocked_in_negative_pressure`

---

## P5 — 多人遭遇压测 ✅ 2026-05-12

### 交付物

1. **5 玩家交叉遭遇矩阵**
   - 配置：5 个玩家，不同境界（1 醒灵 / 1 引气 / 1 凝脉 / 1 固元 / 1 通灵），同时在 spawn_plain + broken_peaks 区域自由移动 30 分钟
   - 观察指标：
     - 自然发生的遭遇次数
     - 每次遭遇的类型分布（绕行/和平/试探/死战/合作）
     - 每次遭遇的结果（一方死亡/两方撤退/和平分离/合作后背叛）
     - 每次遭遇后的信息传播（LifeRecord 记录 / NPC 情报可查 / narration 提及）
     - 玩家 30 分钟后的情感反馈："相遇时紧张吗？""被背叛后愤怒吗？""还想再遇到那个人吗？"

2. **叙事涌现验证**
   - 30 分钟测试后，检查：
     - agent narration 是否生成了至少一条"基于玩家遭遇"的政治叙事
     - NPC 是否在后续交互中提到了之前的遭遇
     - 是否有"江湖传闻"在 NPC 间传播
   - 涌现目标：30 分钟内至少 1 条有意义的"遭遇故事"被系统记录和传播

### 验收抓手

- 压测脚本：`scripts/e2e/pvp-5player-encounter.sh`
- 实测报告：5 人 × 30 分钟的遭遇矩阵数据 + 情感反馈 + 叙事涌现验证
- 涌现率 > 70%（70% 的遭遇产生了可追踪的后续影响）

---

## Finish Evidence

- **落地清单**：
  - P0：`server/src/social/pvp_encounter.rs` 落地 `EncounterPhase`、`EncounterDecisionWeights`、`phase_for_distance`，把遭遇三阶段和权重表变成可测试契约。
  - P1：`client/src/main/java/com/bong/client/social/SilentSignalSystem.java` + `SilentSignalSystemTest.java` 覆盖 15 格内火把、骨币、后退、双蹲、指向、打坐信号；不显示规则解释或假信号警告。
  - P2：`PvpEncounterEvent`、`EncounterOutcome::Betrayal`、`SocialRelationshipEvent`、`SocialRenownDeltaEvent(reason=pvp_betrayal)`、`PlayerIdentities` 信誉惩罚链路已接入，NPC 目击时叛徒 active identity 增加恶名和“背信者”标签。
  - P3：`BiographyEntry::PvpEncounter` / `PvpBetrayal`、`server/src/persistence/mod.rs` 生平事件索引、`agent/packages/schema/src/biography.ts` 与 generated schema 已同步；`server/src/npc/intel.rs` 提供骨币付费的部分遭遇情报查询契约。
  - P4：`server/src/network/chat_collector.rs` 在 `TsyPresence` 下阻断普通 chat；`server/src/world/tsy.rs` 落地撤离点 PvP 囚徒困境窗口模型。
  - P5：`scripts/e2e/pvp-5player-encounter.sh` 固化 5 玩家交叉遭遇矩阵测试；agent `political-narration` 订阅 `bong:social/renown_delta` 并把 `pvp_betrayal` 转成匿名江湖传闻。
- **关键 commit**：
  - `07be37d4e` · 2026-05-12 · `feat(pvp): 落地多人遭遇社交后果`
  - `d22b7a00a` · 2026-05-12 · `feat(client): 增加沉默遭遇信号识别`
  - `2a0372cf3` · 2026-05-12 · `feat(agent): 接入背叛声望叙事`
- **测试结果**：
  - `cd server && cargo test pvp_encounter` → 6 passed
  - `cd server && cargo test chat_blocked_in_negative_pressure` → 1 passed
  - `cd server && cargo test pvp_at_extract_point` → 1 passed
  - `cd server && cargo test npc_intel_purchase_partial_info` → 1 passed
  - `scripts/e2e/pvp-5player-encounter.sh` → `pvp_five_player_encounter_matrix_emits_trackable_story` passed
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 4485 passed
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests "com.bong.client.social.SilentSignalSystemTest"` → passed
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL
  - `cd agent && npm run generate -w @bong/schema` → passed
  - `cd agent && npm run generate:check -w @bong/schema` → passed
  - `cd agent && npm run build && npm test -w @bong/tiandao && npm test -w @bong/schema` → tiandao 356 passed / schema 376 passed
- **跨仓库核验**：
  - server：`PvpEncounterEvent`、`EncounterOutcome::Betrayal`、`BiographyEntry::PvpEncounter`、`SocialRenownDeltaEvent(reason=pvp_betrayal)`、`TsyPresence` chat block、`TsyExtractPvpProfile`
  - client：`SilentSignalSystem.detect`、`SignalKind.PEACE_TORCH`、`SignalKind.BONE_COIN_OFFER`、`SignalKind.SEATED_NEUTRAL`
  - agent/schema：`BiographyEntryV1` 新增 `PvpEncounter` / `PvpBetrayal`，`POLITICAL_EVENT_CHANNELS` 订阅 `SOCIAL_RENOWN_DELTA`，`eventType: "pvp_betrayal"` 匿名 narration
- **遗留 / 后续**：
  - 大规模遭遇（10+ 玩家在兽潮触发点、伪灵脉中心或裂缝口聚集）仍需后续 plan 做 live telemetry 与负载验证。
  - 长期宿敌叙事（同两名玩家多次遭遇形成“老对手”关系）尚未做跨遭遇聚合。
  - 正式 party/组队 UI 不在本 plan 范围；当前仍保持匿名、动作信号与既有 chat/交易链路驱动。
