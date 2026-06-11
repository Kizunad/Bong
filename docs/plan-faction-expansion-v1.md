# Bong · plan-faction-expansion-v1 · active

将 NPC 派系从三档匿名占位（Attack / Defend / Neutral）扩展为**具名散修势力**——每个势力有名称、区域锚点（地盘）、宗门残余背景、内部等级、**领袖 NPC**，并提供 v2 后续可消费的"派系事件 API"（结盟 / 宣战 / 领袖陨落 / 势力消亡）。worldview §十一 的"散修江湖人来人往 / 势力消长"终于有具体载体。

**来源**：`plan-faction-wars-v1` 骨架 §8 开放问题 #1："v2 是否扩展为具名散修势力（青云残峰猎人会 / 沧渊盐商帮等）"及反向被依赖注记 `plan-faction-expansion-v1`（待立，具名散修势力 + 领袖 NPC）

**前置条件**：
- `plan-faction-wars-v1` P2 完成（Zone 控制权 + `FactionWarOutcome` 落地）——具名派系须与战争结算互联

**交叉引用**：`plan-npc-ai-v1` ✅（big-brain Scorer/Action + `FactionStore` / `FactionMembership` + `assign_hostile_encounters`）· `plan-npc-virtualize-v1` ✅（dormant SoA + 派系 zone 持久化）· `plan-npc-virtualize-v3` ⬜ 骨架（dormant 批量战争推演的参与方是具名派系）· `plan-faction-wars-v1` ⬜ 骨架（战争生命周期，本 plan 硬前置）· `plan-social-v1` ✅（玩家 ↔ NPC 派系挂靠 §5 / Renown 接口）· `plan-social-v2` ⬜ 骨架（战争结果 → 声望联动，本 plan 为 social-v2 提供 `NamedFaction` 类型）· `plan-narrative-political-v1` ✅（派系叙事五核心事件，本 plan 为其提供具名主语；feud/pact 升级为"具名派系 vs 具名派系"）· `plan-qi-physics-v1` ✅（守恒律；派系 zone 控制权影响 `regen_from_zone` 修正系数）

**worldview 锚点**：
- **§十一:937-970 身份与信誉 / NPC 反应分级**：具名派系 = NPC 信誉度的"分母"；不同势力 NPC 对玩家的反应阈值各异
- **§十一:947-970 散修江湖**：NPC 没有灵龛，靠追踪高灵气浓度生存；派系 = 有共同利益/仇恨的散修临时联盟（"人来人往"说明派系流动性强，不是永久宗门）
- **§十一 匿名系统**："没有宗门收你"指玩家**主世界开局**无归属，但玩家可挂靠具名散修势力（沿用 social-v1 §5 挂靠规则）；派系成员身份可隐藏，被识破会改变 NPC 信誉度
- **§九 交易生态**：具名势力垄断特定地盘的灵草/矿脉 → 控制资源供给链 → 产生交易博弈（沧渊商会的骨币流通即此机制载体）
- **§三:124 NPC 与玩家平等**：具名派系 NPC（含领袖）遵守相同的境界/真元规则，不是作弊的 boss；势力消亡 = 所有成员 dormant 耗尽
- **§二 守恒律**：派系控制 zone 影响灵气吸收分配，不创生灵气

**qi_physics 锚点**：
- 具名派系控制 zone 时 `regen_from_zone` 修正系数沿用 faction-wars 的 `ZoneSpiritBonus { faction: NamedFactionId, bonus: f64 }`——改变分配比例，不改变总量
- 派系 NPC（含领袖）战死灵气释放：`qi_physics::qi_release_to_zone`（继承 npc-virtualize-v3 / faction-wars 规则，不重复实现）

---

## 接入面 Checklist

- **进料**：`FactionStore`（现有 Attack/Defend/Neutral）+ `FactionMembership`（NPC 归属）+ `FactionWarOutcome`（faction-wars P2 产出）+ `ZoneRegistry`（zone 坐标/名称）+ `NpcTemplate`（领袖 NPC spawn 模板）+ `Renown { fame, notoriety }`（social-v1 玩家声誉）
- **出料**：`NamedFaction { id, display_name, zone_anchor, lore_tag, current_npc_count, is_active }` 注册表 + `NamedFactionId` enum（新增具名变体）+ `NamedFactionLeader` Component（领袖 NPC entity 标记）+ `FactionZoneClaim`（地盘绑定）+ `FactionRelationMatrix`（势力间敌对/中立/盟约状态）+ `NamedFactionLeaderDownEvent` / `NamedFactionDecayEvent`（领袖陨落 / 势力消亡 event，给 agent narration 消费）
- **共享类型**：`FactionId` 现有 Attack/Defend/Neutral **不删**——新建 `NamedFactionId` enum，war 系统里 `FactionId` 从 `NamedFactionId` 动态映射（各势力可被标记为战争中的 Attacker / Defender）；长远目标是 `FactionId` 成为 `NamedFactionId` 的包装
- **跨仓库契约**：server `bong:faction_state`（named faction 注册表 + 势力血量 + 领袖存活 Redis 快照）；agent 消费 `NamedFactionLeaderDownEvent` / `NamedFactionDecayEvent` 产生"领袖陨落 / 某势力消亡"叙事；client 无新 CustomPayload（复用 faction-wars HUD layer，只换 display_name 字符串）
- **worldview 锚点**：§十一 NPC 信誉分级 + 散修江湖 + 匿名系统 + §九 交易生态 + §三 NPC 平等
- **qi_physics 锚点**：`ZoneSpiritBonus` 修正系数（faction-wars 已实装，本 plan 只扩展 faction 主语类型）

---

## §0 设计轴心

末法残土没有传统宗门，但散修靠利益聚合出**三到五个具名势力**：
- 每个势力有 **zone 锚点**（主要活跃地盘）、**利益方向**（猎杀/贸易/勘探/护矿）、**残存背景**（失落宗门余脉 / 草台商帮 / 寡头猎队）、**领袖 NPC**（少数势力可为"无头"散盟，见下表）
- 势力内部只有两档：`FactionRank::Leader`（头目，对应 `NamedFactionLeader` 标记 entity）和 `FactionRank::Disciple`（普通成员）——与现有 enum 对齐，不引入新层级
- 势力之间由 `FactionRelationMatrix` 维护二元关系（Hostile / Neutral / Pact），用于 dormant 批量战斗触发条件 + `assign_hostile_encounters` 权重
- 领袖 NPC 按 §三 与玩家相同的境界/真元规则运作；领袖死亡走 `FactionStatus::Headless`（无头态，**不自动选举**，留 v2），势力 NPC 全部耗尽才走 `NamedFactionDecayEvent` 消亡
- **v1 不改变玩家挂靠规则**（沿用 social-v1 §5）——只让玩家能挂靠到"有名字的势力"而非 Attack/Defend 占位符

**v1 预设具名势力（3 个，可随运营扩展）**：

| 势力 ID | 显示名 | zone 锚点 | 利益方向 | 领袖（境界） | 背景梗概 |
|---------|--------|---------|---------|------------|---------|
| `QingyunHunters` | 青云猎盟 | 青云残峰 | 猎杀野兽、护矿收租 | 盟主（固元上阶） | 失落宗门护山堂的散修残余，靠收保护费维系松散组织 |
| `CangyuanMerchants` | 沧渊商会 | 裂谷 / 血谷 | 灵石贸易、骨币流通 | 会首（通灵初阶·隐藏） | 原盐商商队演化，以货仓为据点，不主动打人但见利翻脸；垄断地盘灵草矿脉供给链 |
| `NorthWasteDrifters` | 北荒漂流者 | 北荒 / 灵泉沼 | 坍缩渊探查、遗物倒卖 | 无常设领袖（开局 `Headless`） | 无根散修的临时联盟，成员替换率极高，无常设领袖——天然 Headless，验证无头态分支 |

> 三势力 zone 锚点对应已落地的 terrain profile（青云残峰 / 裂谷·血谷 / 北荒·灵泉沼），不引入新坐标系。具体领袖名/历史需对照 worldview + library 书籍后确认；上表为占位草案，P0 前必须收口。

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-06-11 | `NamedFactionId` 注册表 + 数据模型 + migration | `NamedFactionId` PR 合并 + 现有 FactionId 代码无 breakage + schema 双端校验 |
> **P0 落地（2026-06-11）**：`server/src/npc/faction.rs` 追加 `NamedFactionId`（青云猎盟/沧渊商会/北荒漂流者）+ `NamedFaction`/`FactionStatus`(Active/Headless/Decayed)/`NamedFactionRegistry`（`register()` 启动真注册 3 条，北荒初始 Headless）+ `FactionStore::faction_id_for_war`（兼容层真接现有 `is_hostile_pair`，war 逻辑零改）。schema：**避命名撞车**——已有 `FactionStateV1`/`bong:faction_state`（emergent group census 在用），故新增 `NamedFactionStateV1` + `bong:named_faction_state`（TypeBox+Rust serde+3 sample 双端）。migration **v30**：现有 `FactionMembership` Attack→qingyun/Defend→cangyuan/Neutral→north_waste 按 zone_anchor 真迁移。**lore 正典核验**：青云猎盟（docs/library 宗门残息）/ 北荒漂流者 Headless（北荒坍缩渊记"入者十归者不过百"强支撑）；**沧渊商会正典无直接记载 → 代码注释如实标注 plan 设定/推演自血谷矿脉经济，非硬编乱猜**。leader 具名正典无确指，P0 不设 leader 字段（P2 领袖 spawn 再填）。**遗留**：下游 social-v2(`WarReputationRecord`)/faction-wars(`FactionWarEvent`) 消费 `NamedFactionId`（契约已留）；P1（`FactionRelationMatrix` + dormant 权重接入）+ P2（领袖/census）待续。cargo fmt+clippy(-D warnings)+test **8536 passed** + agent schema 双端（3 sample 正反对拍）。

| **P1** | ✅ 2026-06-11 | `FactionRelationMatrix` + are_hostile + scorer 偏置 + /faction list（基座相位） | 矩阵/接线/14 单测落地并接入 `assign_hostile_encounters`；**runtime 激活随 P2 `FactionZoneClaim`** |
> **P1 落地（2026-06-11，基座相位）**：`server/src/npc/faction.rs` 新增 `FactionRelation`(Hostile/Neutral/Pact) + `FactionRelationMatrix`(三对初值，`are_hostile` 对称，未注册对组默认 Neutral) + `NamedFactionMembership` component；`assign_hostile_encounters`（真注册 Update system）双方携 membership 时走矩阵、否则 fallback 旧 `is_hostile_pair`；`faction_duel_scorer_system` 关系偏置(Hostile +0.2 / Pact -0.3)；`/faction list` 显示关系矩阵。14 新单测（cargo fmt+clippy(-D warnings)+test **8615 passed / 0 failed**）。**⚠️ 基座相位，runtime 暂不驱动真实战斗**：① production NPC 尚未附 `NamedFactionMembership`（spawn 散落各 archetype，须 P2 `FactionZoneClaim` 按 `zone_anchor` 绑定后才有 NPC 携带）；② dormant 批量战斗真实路径 `collect_zone_combat_pairs` 走 `EmergentGroupId` 维度，接 `NamedFactionId` 需 P2 给 dormant 快照加字段；③ schema `FactionRelationEntryV1` 仍 P0 二值 `hostile: bool`，三值同步留 P2（当前无 live publisher，不丢数据）。矩阵已被真实 system 消费且被 14 测试锁定，**激活数据入口（membership 附加）归 P2**。
| **P2** | ⬜ | 领袖 NPC spawn + `FactionZoneClaim` 地盘绑定 + 领袖行为树 | 领袖 spawn 于正确 zone；FactionZoneClaim 与 FactionStore 一致；领袖 big-brain scorer 在地盘内激活 |
| **P3** | ⬜ | 玩家挂靠具名势力 + NPC 信誉分组 + 领袖陨落/势力消亡 + agent 叙事 | 挂靠后 zone NPC 信誉正确分组；领袖死亡 → Headless + agent narration；NPC 清零 → 消亡 narration |

---

## P0 — 注册表 + 数据模型

- [ ] `NamedFactionId` enum（`server/src/npc/faction.rs`）：
  - 变体：`QingyunHunters / CangyuanMerchants / NorthWasteDrifters`
  - `impl NamedFactionId { fn display_name(&self) -> &str; fn zone_anchor(&self) -> ZoneId; fn lore_tag(&self) -> &str }`
- [ ] `NamedFaction` struct（注册表条目）：`{ id: NamedFactionId, display_name, zone_anchor: ZoneId, current_npc_count: u32, status: FactionStatus, is_active: bool }`
- [ ] `FactionStatus` enum（`server/src/npc/faction.rs`）：`Active / Headless / Decayed`——领袖存活态机
- [ ] `NamedFactionRegistry` resource（`server/src/npc/faction.rs`）：启动时按常量注册 3 条记录（北荒漂流者初始 `Headless`）
- [ ] **FactionId 兼容层**：`FactionId::Attack/Defend/Neutral` 保留；新增 `fn faction_id_for_war(a: NamedFactionId, b: NamedFactionId) -> (FactionId, FactionId)` 动态映射（战争发起方 → Attacker，防御方 → Defender）
- [ ] **schema**：新增 `FactionStateV1`（TypeBox source of truth → JSON Schema → Rust serde）；`agent/packages/schema/samples/` 配正反 sample 双端对拍
- [ ] **Migration**：现有持久化 `FactionMembership { faction: FactionId }` 中 Attack/Defend/Neutral NPC 按 zone_anchor 迁移到对应 `NamedFactionId`（数据转换 script，`server/src/npc/faction_migration.rs`）
- [ ] `bong:faction_state` Redis key 结构：`{ named_factions: [...], relation_matrix: [...] }` JSON 快照（agent 可读）
- [ ] ≥ 8 单测（`NamedFactionId` display_name / zone_anchor 正确 · `FactionStatus` 三变体 · migration 三种 FactionId 各自映射到正确具名势力 · FactionId 兼容层 war 映射可逆 · `FactionStateV1` serde 向后兼容 / sample 双端校验）

**P0 验收**：PR 合并 + 8 单测 green + `cargo clippy` 无新 warning + schema sample 双端校验通过

---

## P1 — FactionRelationMatrix + dormant 权重

- [ ] `FactionRelationMatrix` resource（`server/src/npc/faction.rs`）：
  - `relations: HashMap<(NamedFactionId, NamedFactionId), FactionRelation>`
  - `FactionRelation` enum：`Hostile / Neutral / Pact`
  - v1 初始值：`(QingyunHunters, CangyuanMerchants) = Neutral`；`(QingyunHunters, NorthWasteDrifters) = Hostile`；`(CangyuanMerchants, NorthWasteDrifters) = Neutral`
  - `fn are_hostile(a, b) -> bool` 供 dormant 批量战斗 system 调用（对称处理单向关系）
- [ ] npc-virtualize-v3 `dormant_batch_combat_system` 调用 `are_hostile` 替换原有 `is_hostile_pair(FactionId::Attack, FactionId::Defend)` 硬编码
- [ ] `assign_hostile_encounters` scorer 权重：`Hostile` 关系 +20 hostile bias；`Pact` 关系 -30（不会主动攻击盟友）
- [ ] ≥ 10 单测（三对关系组合正确 · Hostile 对组触发 dormant 战斗 · Pact 对组不触发 · 单向关系对称处理 · `Headless` 势力仍按关系矩阵参与战斗）

**P1 验收**：`/faction list` 显示三势力关系矩阵；dormant 批量战斗只在 Hostile 对组间触发

---

## P2 — 领袖 NPC + 地盘绑定 + 行为树

- [ ] `NamedFactionLeader` Component（`server/src/npc/faction.rs`）：标记领袖 entity，`{ faction: NamedFactionId }`
- [ ] 领袖 spawn：每个 `Active` 势力按 `NpcTemplate`（unique 模板，境界见 §0 表）在 zone_anchor spawn 一名领袖；`Headless` 势力（北荒漂流者）不 spawn 领袖
- [ ] `FactionZoneClaim` Component / resource（`server/src/npc/faction.rs`）：`{ faction: NamedFactionId, zone: ZoneId }`——地盘绑定，须与 `FactionStore` / `NamedFactionRegistry.zone_anchor` 一致
- [ ] 领袖 big-brain 行为树（`server/src/npc/ai/`）：Scorer/Action——巡逻地盘 / 征收过路费（向地盘内非本派系 NPC/玩家）；scorer 仅在 `FactionZoneClaim` 范围内激活
- [ ] ≥ 10 单测（领袖 spawn 于正确 zone · `Headless` 势力不 spawn 领袖 · `FactionZoneClaim` 与 registry 锚点一致 · 领袖 scorer 在地盘内激活 / 出地盘失活 · 领袖境界符合 §0 表）

**P2 验收**：集成测试——领袖 NPC spawn 于正确 zone；`/faction list` 显示领袖存活；领袖在地盘内触发巡逻/收费行为，北荒漂流者无领袖

---

## P3 — 玩家挂靠 + NPC 信誉分组 + 领袖陨落/势力消亡 + agent 叙事

- [ ] `FactionReputation` component（`server/src/social/`）：
  - `per_faction: HashMap<NamedFactionId, i32>`（初始 0，区间 -100..=100）
  - 与现有 `Renown { fame, notoriety }` **并行**，不替换——`Renown` 是个人声誉，`FactionReputation` 是派系信誉
- [ ] 玩家挂靠逻辑扩展：`/faction join <named_faction_id>`（从原有 Attack/Defend 命令改为具名）；挂靠条件：对应势力 `per_faction >= 0`（中性以上才可挂靠）+ 势力 `status != Decayed`
- [ ] NPC 信誉分组：zone NPC 查 `per_faction[zone_anchor_faction]` 而非全局 Renown；交互时走 social-v1 §5 高/中/低反应分级（阈值：>50 高，10-50 中，-10~10 正常，<-10 低，<-50 通缉）；高信誉 → 折扣/情报/私活对话分支
- [ ] `FactionReputationDelta` 事件（`server/src/social/events.rs`）：`{ player, faction: NamedFactionId, delta: i32, reason: &str }`；战争结算由 faction-wars `faction_war_settle_system` 发送
- [ ] `NamedFactionLeaderDownEvent { faction, zone }`：领袖死亡 emit → `status = Headless`（不自动选举）；agent narration（broadcast，style: narrative）
- [ ] `NamedFactionDecayEvent { faction, final_zone, last_npc_count }`：`current_npc_count` 归零 emit → `status = Decayed` / `is_active = false`；挂靠玩家 `FactionMembership` 清空（role → None）；NPC 信誉 `per_faction[faction]` 冻结（与 social-v1 identity 冻结语义对齐）
- [ ] agent 叙事消费 `bong:faction_state`：
  - 领袖陨落："青云猎盟的盟主死在血谷口——残峰上群龙无首，过路费再没人收了。"
  - 势力消亡："青云猎盟的最后一支队伍在血谷覆灭——残峰上再无那面破旗帜飘动。"
- [ ] ≥ 10 单测（挂靠条件含 status 校验 · per_faction 更新 · NPC 信誉分组正确使用 zone 对应势力 · 与全局 Renown 无互相污染 · 领袖死亡 → Headless event · NPC 清零 → Decay event + membership 清空 + Redis 快照 status/is_active 正确）

**P3 验收**：e2e——挂靠 QingyunHunters → 青云残峰 NPC 正常交易、沧渊 NPC 无变化；运营命令杀领袖 → `NamedFactionLeaderDownEvent` → agent narration；清空势力 NPC → `NamedFactionDecayEvent` → agent narration

---

## §8 开放问题（P0 决策门前需收口）

1. **FactionId 长期迁移**：v1 保留 Attack/Defend/Neutral 做兼容层，v2 是否彻底废弃并只用 `NamedFactionId`？接受有运营风险（现存持久化数据需二次 migration）
2. **领袖死亡后处理**：v1 简化为 `Headless`（不自动选举）；v2 是否支持"推举新领袖"或"势力崩溃"？北荒漂流者天然 Headless，可作为无头态长期运行的样本
3. **势力新生**：势力消亡（`Decayed`）后能否"新生"（达到条件 spawn 一批 NPC 重建）？v1 不实装，留接口 `NamedFactionRegistry::revive()`
4. **玩家基地型势力**：玩家自建"宗门"是否在此 plan 扩展？**明确 Not In Scope**——worldview §十一 "没有宗门收你"，玩家势力留独立 plan
5. **关系矩阵动态变化**：`FactionRelation` 目前静态初始化；战争胜负是否自动改变关系（Hostile → Neutral 后签约）？v1 人工 patch，v2 联动 faction-wars 结算
6. **具名势力数量**：v1 三个够吗？`NorthWasteDrifters` 北荒 zone 本身 NPC 稀少，是否改为另一个 zone 更密集的势力？待 worldgen 数据确认再拍

**P0 启动前必须**：通过 Explore agent 核查 npc-virtualize-v3 dormant 批量战斗 P0-P1 是否已合并，确认 `DormantCombatOutcome` 接口稳定；核查 social-v1 §5 挂靠存储路径（`server/src/social/` `FactionMembership` 落盘格式）；核查 npc-ai-v1 `NpcTemplate` / big-brain Scorer 接口（领袖行为树挂载点）。全部收口才能开 P0。
