# Bong · plan-faction-expansion-v1 · 骨架

将 NPC 派系从三档匿名占位（Attack / Defend / Neutral）扩展为**具名散修势力**——每个势力有名称、区域锚点、宗门残余背景、内部等级，并提供 v2 后续可消费的"派系事件 API"（结盟 / 宣战 / 消亡）。worldview §十一 的"势力消长"终于有具体载体。

**来源**：`plan-faction-wars-v1` 骨架 §8 开放问题 #1："v2 是否扩展为具名散修势力（青云残峰猎人会 / 沧渊盐商帮等）"及反向被依赖注记 `plan-faction-expansion-v1`（待立）

**前置条件**：
- `plan-faction-wars-v1` P2 完成（Zone 控制权 + `FactionWarOutcome` 落地）——具名派系须与战争结算互联

**交叉引用**：`plan-npc-virtualize-v1` ✅（`FactionStore` / `FactionMembership` / dormant SoA）· `plan-faction-wars-v1` ⬜ 骨架（战争生命周期，本 plan 硬前置）· `plan-social-v1` ✅（玩家 ↔ NPC 派系挂靠 §5 / Renown 接口）· `plan-social-v2` ⬜ 骨架（战争结果 → 声望联动，本 plan 为 social-v2 提供 `NamedFaction` 类型）· `plan-narrative-political-v1` ✅（派系叙事五核心事件，本 plan 为其提供具名主语）· `plan-qi-physics-v1` ✅（守恒律；派系 zone 控制权影响 `regen_from_zone` 修正系数）· `plan-npc-ai-v1` ✅（big-brain / assign_hostile_encounters）

**worldview 锚点**：
- **§十一:937-970 身份与信誉 / NPC 反应分级**：具名派系 = NPC 信誉度的"分母"；不同势力 NPC 对玩家的反应阈值各异
- **§十一 匿名系统**："没有宗门收你"指玩家**主世界开局**无归属，但玩家可挂靠具名散修势力（沿用 social-v1 §5 挂靠规则）
- **§三:124 NPC 与玩家平等**：具名派系 NPC 遵守相同规则；势力消亡 = 所有成员 dormant 耗尽
- **§二 守恒律**：派系控制 zone 影响灵气吸收分配，不创生灵气

**qi_physics 锚点**：
- 具名派系控制 zone 时 `regen_from_zone` 修正系数沿用 faction-wars 的 `ZoneSpiritBonus { faction: NamedFactionId, bonus: f64 }`——改变分配比例，不改变总量
- 派系 NPC 战死灵气释放：`qi_physics::qi_release_to_zone`（继承 npc-virtualize-v3 / faction-wars 规则，不重复实现）

---

## 接入面 Checklist

- **进料**：`FactionStore`（现有 Attack/Defend/Neutral）+ `FactionMembership`（NPC 归属）+ `FactionWarOutcome`（faction-wars P2 产出）+ `ZoneRegistry`（zone 坐标/名称）+ `Renown { fame, notoriety }`（social-v1 玩家声誉）
- **出料**：`NamedFaction { id: NamedFactionId, display_name, zone_anchor, lore_tag }` 注册表 + `NamedFactionId` enum（新增具名变体）+ `FactionRelationMatrix`（势力间敌对/中立/盟约状态）+ `NamedFactionDecayEvent`（势力消亡 event，给 agent narration 消费）
- **共享类型**：`FactionId` 现有 Attack/Defend/Neutral **不删**——新建 `NamedFactionId` enum，war 系统里 `FactionId` 从 `NamedFactionId` 动态映射（各势力可被标记为战争中的 Attacker / Defender）；长远目标是 `FactionId` 成为 `NamedFactionId` 的包装
- **跨仓库契约**：server `bong:faction_state`（named faction 注册表 + 势力血量 Redis 快照）；agent 消费 `NamedFactionDecayEvent` 产生"某势力消亡"叙事；client 无新 CustomPayload（复用 faction-wars HUD layer，只换 display_name 字符串）
- **worldview 锚点**：§十一 NPC 信誉分级 + 匿名系统 + §三 NPC 平等
- **qi_physics 锚点**：`ZoneSpiritBonus` 修正系数（faction-wars 已实装，本 plan 只扩展 faction 主语类型）

---

## §0 设计轴心

末法残土没有传统宗门，但散修靠利益聚合出**三到五个具名势力**：
- 每个势力有 **zone 锚点**（主要活跃区域）、**利益方向**（猎杀/贸易/勘探/护矿）、**残存背景**（失落宗门余脉 / 草台商帮 / 寡头猎队）
- 势力内部只有两档：`FactionRank::Leader`（少数头目）和 `FactionRank::Disciple`（普通成员）——与现有 enum 对齐，不引入新层级
- 势力之间由 `FactionRelationMatrix` 维护二元关系（Hostile / Neutral / Pact），用于 dormant 批量战斗触发条件 + assign_hostile_encounters 权重
- **v1 不改变玩家挂靠规则**（沿用 social-v1 §5）——只让玩家能挂靠到"有名字的势力"而非 Attack/Defend 占位符

**v1 预设具名势力（3 个，可随运营扩**）：

| 势力 ID | 显示名 | zone 锚点 | 利益方向 | 背景梗概 |
|---------|--------|---------|---------|---------|
| `QingyunHunters` | 青云猎盟 | 青云残峰 | 猎杀野兽、护矿收租 | 失落宗门护山堂的散修残余，靠收保护费维系松散组织 |
| `CangyuanMerchants` | 沧渊商会 | 裂谷/血谷 | 灵石贸易、骨币流通 | 原盐商商队演化，以货仓为据点，不主动打人但见利翻脸 |
| `NorthWasteDrifters` | 北荒漂流者 | 北荒 / 灵泉沼 | 坍缩渊探查、遗物倒卖 | 无根散修的临时联盟，成员替换率极高，无常设领袖 |

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | `NamedFactionId` 注册表 + 数据模型 + migration | `NamedFactionId` PR 合并 + 现有 FactionId 代码无 breakage |
| **P1** | ⬜ | `FactionRelationMatrix` + dormant 权重接入 | 三势力在 npc-virtualize-v3 批量战斗中按关系矩阵出现正确敌对对 |
| **P2** | ⬜ | 玩家挂靠到具名势力 + NPC 信誉分组 | 挂靠 `QingyunHunters` 后青云残峰 NPC 信誉度正确上升，沧渊 NPC 不变 |
| **P3** | ⬜ | `NamedFactionDecayEvent` + agent 叙事 + player 挂靠 HUD | agent 消费势力消亡 event 生成叙事 |

---

## P0 — 注册表 + 数据模型

- [ ] `NamedFactionId` enum（`server/src/npc/faction.rs`）：
  - 变体：`QingyunHunters / CangyuanMerchants / NorthWasteDrifters`
  - `impl NamedFactionId { fn display_name(&self) -> &str; fn zone_anchor(&self) -> ZoneId; fn lore_tag(&self) -> &str }`
- [ ] `NamedFaction` struct（注册表条目）：`{ id: NamedFactionId, display_name, zone_anchor: ZoneId, current_npc_count: u32, is_active: bool }`
- [ ] `NamedFactionRegistry` resource（`server/src/npc/faction.rs`）：启动时按常量注册 3 条记录
- [ ] **FactionId 兼容层**：`FactionId::Attack/Defend/Neutral` 保留；新增 `fn faction_id_for_war(a: NamedFactionId, b: NamedFactionId) -> (FactionId, FactionId)` 动态映射（战争发起方 → Attacker，防御方 → Defender）
- [ ] **Migration**：现有持久化 `FactionMembership { faction: FactionId }` 中 Attack/Defend/Neutral NPC 按 zone_anchor 迁移到对应 `NamedFactionId`（数据转换 script，`server/src/npc/faction_migration.rs`）
- [ ] `bong:faction_state` Redis key 结构：`{ named_factions: [...], relation_matrix: [...] }` JSON 快照（agent 可读）
- [ ] ≥ 8 单测（NamedFactionId display_name / zone_anchor 正确 / migration 三种 FactionId 各自映射到正确具名势力 / FactionId 兼容层 war 映射可逆）

**P0 验收**：PR 合并 + 8 单测 green + `cargo clippy` 无新 warning

---

## P1 — FactionRelationMatrix + dormant 权重

- [ ] `FactionRelationMatrix` resource（`server/src/npc/faction.rs`）：
  - `relations: HashMap<(NamedFactionId, NamedFactionId), FactionRelation>`
  - `FactionRelation` enum：`Hostile / Neutral / Pact`
  - v1 初始值：`(QingyunHunters, CangyuanMerchants) = Neutral`；`(QingyunHunters, NorthWasteDrifters) = Hostile`；`(CangyuanMerchants, NorthWasteDrifters) = Neutral`
  - `fn are_hostile(a, b) -> bool` 供 dormant 批量战斗 system 调用
- [ ] npc-virtualize-v3 `dormant_batch_combat_system` 调用 `are_hostile` 替换原有 `is_hostile_pair(FactionId::Attack, FactionId::Defend)` 硬编码
- [ ] `assign_hostile_encounters` scorer 权重：`Hostile` 关系 +20 hostile bias；`Pact` 关系 -30（不会主动攻击盟友）
- [ ] ≥ 10 单测（三对关系组合正确 / Hostile 对组触发 dormant 战斗 / Pact 对组不触发 / 单向关系对称处理）

**P1 验收**：`/faction list` 显示三势力关系矩阵；dormant 批量战斗只在 Hostile 对组间触发

---

## P2 — 玩家挂靠 + NPC 信誉分组

- [ ] `FactionReputation` component（`server/src/social/`）：
  - `per_faction: HashMap<NamedFactionId, i32>`（初始 0，区间 -100..=100）
  - 与现有 `Renown { fame, notoriety }` **并行**，不替换——`Renown` 是个人声誉，`FactionReputation` 是派系信誉
- [ ] 玩家挂靠逻辑扩展：`/faction join <named_faction_id>`（从原有 Attack/Defend 命令改为具名）；挂靠条件：对应势力 `per_faction >= 0`（中性以上才可挂靠）
- [ ] NPC 信誉分组：zone NPC 查 `per_faction[zone_anchor_faction]` 而非全局 Renown；交互时走 social-v1 §5 的高/中/低反应分级（阈值：>50 高，10-50 中，-10~10 正常，<-10 低，<-50 通缉）
- [ ] `FactionReputationDelta` 事件（`server/src/social/events.rs`）：`{ player, faction: NamedFactionId, delta: i32, reason: &str }`；战争结算由 faction-wars `faction_war_settle_system` 发送
- [ ] ≥ 10 单测（挂靠条件 / per_faction 更新 / NPC 信誉分组正确使用 zone 对应势力 / 与全局 Renown 无互相污染）

**P2 验收**：e2e — 挂靠 QingyunHunters → 青云残峰 NPC 正常交易；沧渊 NPC 无变化

---

## P3 — 势力消亡 + agent 叙事

- [ ] `NamedFactionDecayEvent { faction: NamedFactionId, final_zone: ZoneId, last_npc_count: u32 }`：当势力 `current_npc_count` 归零时 emit（`server/src/npc/faction.rs`）
- [ ] agent 叙事消费：`bong:faction_state` 订阅，产生势力消亡 narration（broadcast，style: narrative）：
  - "青云猎盟的最后一支队伍在血谷覆灭——残峰上再无那面破旗帜飘动。"
  - "沧渊商会的货仓关门了。有人说是被劫，有人说是主事的人悄悄跑路。"
- [ ] 消亡后处理：`is_active = false`；挂靠玩家 `FactionMembership` 清空（role → None）；NPC 信誉 per_faction[faction] 冻结（不再更新，与 social-v1 identity 冻结语义对齐）
- [ ] ≥ 6 单测（NPC 清零触发 event / is_active 更新 / 挂靠玩家 membership 清空 / Redis 快照 is_active=false）

**P3 验收**：手测——运营命令清空某势力 NPC → `NamedFactionDecayEvent` → agent narration 广播

---

## §8 开放问题（P0 决策门前需收口）

1. **FactionId 长期迁移**：v1 保留 Attack/Defend/Neutral 做兼容层，v2 是否彻底废弃并只用 `NamedFactionId`？接受有运营风险（现存持久化数据需二次 migration）
2. **势力新生**：势力消亡后能否"新生"（达到条件 spawn 一批 NPC 重建）？v1 不实装，留接口 `NamedFactionRegistry::revive()`
3. **三势力以外**：玩家基地型势力（"玩家宗门"）是否在此 plan 扩展？**明确 Not In Scope**——worldview §十一 "没有宗门收你"，玩家势力留独立 plan
4. **关系矩阵动态变化**：`FactionRelation` 目前静态初始化；战争胜负是否自动改变关系（Hostile → Neutral 后签约）？v1 人工 patch，v2 联动 faction-wars 结算
5. **具名势力数量**：v1 三个够吗？`NorthWasteDrifters` 北荒 zone 本身 NPC 稀少，是否改为另一个 zone 更密集的势力？待 worldgen 数据确认再拍

**P0 启动前必须**：通过 Explore agent 核查 npc-virtualize-v3 dormant 批量战斗 P0-P1 是否已合并，确认 `DormantCombatOutcome` 接口稳定；核查 social-v1 §5 挂靠存储路径（`server/src/social/` `FactionMembership` 落盘格式）。全部收口才能开 P0。
