# Bong · plan-dying-elder-v1 · 骨架

**垂死的大能——稀有欺骗性遭遇**——实装 worldview §七 稀有实体"垂死的大能"：化虚境修士被困负灵域濒死，向玩家"交易"（给丹换传承）；但其恢复真元后有一定概率反手夺舍/灭口玩家。正确解法是拖延/布陷阱看其自毙，再搜遗物。这是末法时代"无人可信、算计至上"世界观的具象化高风险遭遇。

## 目标

- 实装 worldview §七「垂死的大能」：负灵域濒死化虚老怪的不完全信息博弈遭遇
- 体现末法「无人可信」世界观：给丹救活有真实代价/收益，但 `betray_probability` 不可观测
- 工程目标：四态状态机 + 守恒律驱动的两种结局（夺舍扣 `qi_max` / 自毙 `qi_release` 让 zone 灵气跃升）
- 验收：遭遇 → 对话 → 路径选择 → 结局 → agent 叙事 完整 e2e，SpiritEye / Renown 影响正确

**来源**：worldview §七 稀有实体"垂死的大能"

**前置条件**：
- `plan-npc-ai-v1` ✅ — big-brain Scorer/Action（行为树，Plea/Betray 状态）
- `plan-cultivation-v1` ✅ — Realm 化虚 + qi_current 耗尽条件
- `plan-qi-physics-v1` ✅ — 负灵域真元被抽干的物理逻辑（守恒律）
- `plan-death-lifecycle-v1` ✅ — 玩家/NPC 死亡链路
- `plan-skill-v1` ✅ — 传承/地阶功法 item（遭遇奖励）
- `plan-social-v1` ✅ — NPC 信誉体系（`Renown` 影响大能初始态度）
- `plan-spirit-treasure-v1` ✅ — 破碎法宝 item 框架（死亡掉落复用，见 §接入面 出料 / P2 loot）

**交叉引用**：`plan-spirit-eye-v1` ✅（神识可感知大能真元回复状态，辅助判断何时会翻脸）· `plan-alchemy-v2` ✅（回元丹作为交易物品，需确认 item_id）· `plan-tsy-v1` ✅（负灵域坍缩渊为最常见遭遇场地）

**worldview 锚点**：
- **§七:744 垂死的大能**："极度稀有的随机事件。被困在 -0.5 负灵域中、即将被抽干的化虚境老怪。他向你'交易'：给他 5 颗回元丹，传你一门地阶功法。陷阱：他恢复真元后第一件事可能是夺舍你或灭口。正确解法——布置陷阱、言语拖延，看着他在负压下崩溃死掉，然后舔包。"
- **§二 负灵域**：spirit_qi = -0.5 → 天地反向抽吸；化虚境真元池大 = 抽得更快（非线性关系）——大能濒死是真实的，不是诈骗
- **§十一 交易与信誉**：信誉 Renown 影响大能初始态度（极高 Renown 玩家可能得到真正的功法，极低则直接翻脸）
- **§九 以物易物**：回元丹 = 游戏内实际流通物资，消耗玩家库存有实际代价

**qi_physics 锚点**：
- 负灵域持续抽大能真元：`qi_physics::negative_zone_drain(elder_entity, zone_spirit_qi=-0.5)` → 每 tick 扣量
- 玩家给丹 → 大能 qi_current 回升：`qi_physics::qi_from_item(elder_entity, huiyuan_dan_qi_value)` + `QiTransfer`
- 大能死亡（真元耗尽）：`qi_physics::qi_release_to_zone`（极大量真元归还 zone，这个 zone 会瞬间灵气跃升——是后续收益的物理来源）
- 大能翻脸后施"夺舍"攻击：扣玩家 qi_max（永久减少容量），走 `QiTransfer { reason: SoulSeize }`

---

## 接入面 Checklist

- **进料**：zone `spirit_qi < DYING_ELDER_ZONE_THRESHOLD`（候选 -0.4）+ 随机低频 spawn timer（极稀 spawn rate，每服务器每 N in-game days 一次）+ `Renown`（玩家声望，影响大能初始反应分支）+ `item.alchemy.huiyuan_dan`（回元丹 item_id）
- **出料**：`DyingElderState` component（Plea / Recovering / Betrayal / Dead）+ `SoulSeizeEvent { target_player, qi_max_drain }` + 死亡后 loot（地阶功法残卷 + 破碎法宝）+ zone spirit_qi 瞬时跃升（大能死亡 qi_release）
- **共享类型**：复用 big-brain Scorer/Action / `QiTransfer` / `Renown`；新增 `DyingElderState` / `SoulSeizeEvent`；掉落复用 skill-v1 地阶功法 + spirit-treasure-v1
- **跨仓库契约**：server emit `bong:elder_encounter` Redis 事件（agent 生成叙事）；client `DyingElderHudLayer`：交易 UI（给丹确认 + 大能真元回复条 + 倒计时警告）；agent narration 叙述"某处有大能气息，看来伤势不轻"（scope: zone perception）
- **worldview 锚点**：§七 垂死的大能 + §二 负灵域 + §十一 交易信誉
- **qi_physics 锚点**：负灵域持续抽真元 / 给丹回真元 / 死亡大量 qi_release / 夺舍扣 qi_max

---

## §0 博弈轴心

遭遇的核心是**不完全信息博弈**：
- 大能确实在濒死（负灵域物理不骗人），所以"给丹救人"有真实代价、也有真实收益（地阶功法）
- 大能恢复真元后的行为取决于 `betray_probability`（大能自身性格参数，生成时随机，玩家不可观测）
- 玩家合理解法：拒绝给丹 → 大能慢慢死 → 搜遗物；或拖延对话 → 同样结果；或布陷阱 → 大能翻脸时反杀
- 高 Renown 玩家：`betray_probability` 降低（世界观上"名声大的人不值得招惹"）
- SpiritEye 可观测大能 qi_current 实时回升 → 辅助判断"快翻脸了"

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | `DyingElderState` 四态模型 + spawn 条件 + 基础 Plea 对话 | 数据模型 PR 合并 + ≥ 8 单测 green |
| **P1** | ⬜ | 给丹交互 + 真元回升 + Betrayal 翻脸 AI | 给丹后 qi_current 回升；`betray_probability` 触发翻脸正确 |
| **P2** | ⬜ | 夺舍攻击（qi_max 永久减少）+ 大能自毙（qi 耗尽）+ 死亡 loot | 两种结局（夺舍/自毙）守恒律均通过；掉落地阶功法 |
| **P3** | ⬜ | client 交互 UI + agent 叙事 + SpiritEye 观测 + Renown 影响 | 完整 e2e：遭遇 → 对话 → 选择路径 → 结局 → agent 叙述 |

---

## P0 — 数据模型 + spawn

- [ ] `DyingElderState` enum（`server/src/fauna/dying_elder.rs`）：`Plea / Recovering(qi_returned: f32) / Betrayal / Dead`
- [ ] `DyingElderBlackboard { state, betray_probability: f32, qi_max_cache: f32, offered_skill_id: SkillId, dan_received: u32 }` component
- [ ] `betray_probability`：生成时按分布随机 `[0.3, 0.95]`（低端 = 真心交易的罕见大能；高端 = 几乎必然翻脸）
- [ ] spawn 条件：`DyingElderSpawnSystem` 低频 timer（每 in-game 30 天一次）+ zone spirit_qi < -0.4 + 区域内无其他大能遭遇（全服同时最多 1 个）
- [ ] spawn 时 `offered_skill_id` 从地阶功法池随机选一个（skill-v1 注册表）
- [ ] ≥ 8 单测（spawn 频率 / spirit_qi 阈值边界 / 全服上限 1 / betray_probability 分布合理性）

---

## P1 — 给丹 + 回真元 + 翻脸

- [ ] 给丹接口：玩家持 `item.alchemy.huiyuan_dan` 对大能 interact → server 消耗 1 颗丹 + `qi_from_item(elder, HUI_YUAN_DAN_QI_VALUE)` + emit `QiTransfer { reason: TradeDan }`；state 转 `Recovering`
- [ ] `DyingElderRecoverSystem`：大能 `qi_current` 每次接受丹后 +X；累计接受 ≥ 5 颗丹 → 进入翻脸判断：`rand() < betray_probability` → `Betrayal`；否则 → `Dead`（信守承诺，给功法后自裁，实现"极罕见的守信结局"）
- [ ] `DyingElderBetrayalAction`（big-brain Action）：激活时 emit `SoulSeizeEvent { target, qi_max_drain: SOUL_SEIZE_DRAIN }` → 玩家 qi_max 永久减少 `SOUL_SEIZE_DRAIN`（候选 10%）+ 攻击玩家 `qi_current = 0`（全扣）
- [ ] ≥ 12 单测（给丹计数 / betray_probability 触发概率分布 / SoulSeizeEvent 守恒律 / 守信结局边界）

---

## P2 — 自毙 + 夺舍 + 完整死亡链路

- [ ] 大能自然死亡（qi 耗尽，不给丹路线）：负灵域 `negative_zone_drain` 每 tick 扣；qi_current → 0 → `Dead` 状态 → `qi_release_to_zone`（化虚级真元量约 500，zone spirit_qi 瞬时大幅跃升）；生成 loot
- [ ] 大能翻脸被反杀：玩家击杀 Betrayal 态大能 → 同上死亡链路 + 额外掉落（翻脸死亡 loot table 稍差，世界观上"大能预期要走所以没带好货"）
- [ ] `DyingElderDeathSystem`：统一处理两种死亡路线 loot 生成（地阶功法残卷 + 破碎法宝）
- [ ] ≥ 10 单测（两条死亡路线 qi release 守恒 / loot 按死亡原因分档 / zone spirit_qi 跃升可观测 / SoulSeize 不超过 qi_max 上限）

---

## P3 — Client UI + Agent 叙事 + SpiritEye + Renown

- [ ] `DyingElderHudLayer`（client）：遭遇时显示大能真元回复条 + "给丹 / 拒绝 / 拖延" 三选一按钮 + 翻脸概率提示（SpiritEye 激活时显示具体百分比，否则只显示"气息有异"模糊提示）
- [ ] SpiritEye 整合：激活时玩家可见 `betray_probability` 数值（"感知到这双眼里藏着多少'仁慈'"）
- [ ] Renown 影响：玩家 `Renown.fame > 300` → 大能 `betray_probability -= 0.2`（世界观上名声太响不值得招惹）
- [ ] agent 叙事消费 `bong:elder_encounter` event：
  - 触发时（zone perception）："某处有强大气息在衰竭，像一颗将熄的火焰——混着求生的执念。"
  - 大能死亡（broadcast）："气息消散了。不知是那位大能心服口服，还是玩家够狠。"
- [ ] ≥ 5 e2e 测试（完整交互 e2e / SpiritEye 激活显示概率 / Renown 门槛影响 / agent 叙事触发）

---

## §8 开放问题（P0 决策门收口）

1. **spawn 频率**：全服 30 天一次是否太少（玩家一两周内都不遇到）？建议与运营数据结合——宁少勿滥，稀少才显珍贵
2. **SoulSeize 扣 qi_max 是否过重**：10% qi_max 永久减少 = 对某些玩家近乎废人，考虑改为"扣本次 session qi_max，下次重登恢复"
3. **守信结局**：大能给了功法自裁 = 世界观上"极罕见"，概率设为 `1 - betray_probability`；需确认"守信死亡"的 loot 是否应更好（"信守承诺的大能更高贵"）
4. **布陷阱的实现**：worldview 原文说"布置陷阱"——但当前 plan 没有实装 trap 系统（plan-daozhan-v1 里有 SpiderTrapPotential）；v1 只允许"拒绝 + 等死"，v2 才接入陷阱交互
5. **遭遇场地**：负灵域 spirit_qi < -0.4 包括坍缩渊（-0.3 ~ -1.2）和负灵风暴；是否只在坍缩渊 spawn（更符合"被困"的叙事）还是野外负灵域也可能？建议 v1 只在坍缩渊 spawn，野外留 v2
