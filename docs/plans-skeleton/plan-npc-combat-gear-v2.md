# plan-npc-combat-gear-v2 — NPC 装备复活：v1 孤儿代码接线补全 + 类玩家背包决策门

> **骨架**（2026-07-03）。一句话：`plan-npc-combat-gear-v1` 的 `NpcEquipment` 整套模型（配装/掉落/视觉合并）已完成却**从未被任何 spawn 路径 insert、战斗结算不读、掉落不联动、快照不持久**——v2 先把这五段断线全部接活（NPC 手持真武器、穿甲真减伤、死亡掉真装备），再以决策门形式收"NPC 类玩家背包"的架构拍板。
>
> **为何开 v2 不并入 v1**：v1 已在 `docs/finished_plans/` 且自报 ✅，但其 P0 承诺的 spawn 接线与 `sync_npc_equipment_system` 实际缺失（文档与代码不符，属 ⚠️ 红旗）。v1 归档文档本 PR 不动（一 PR 一 plan），差异在本节登记，回标交人工或升 active 时处理。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | spawn insert（五段断线之首） | ⬜ |
| P1 | 战斗接线（攻方武器 / 防方护甲） | ⬜ |
| P2 | 视觉接线（merge_equipment 替换 rank 标记物） | ⬜ |
| P3 | 掉落联动 + 战损耐久 | ⬜ |
| P4 | dormant 快照持久化 | ⬜ |
| P5 | 决策门：类玩家背包（B 组，独立拍板） | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：现成孤儿代码全部复用不重造——`NpcEquipment` / `NpcEquipSlot`（`npc/equipment.rs:56-82`）、`assign_npc_equipment`（`equipment.rs:275-302`，按 archetype/realm/faction 确定性配装）、`npc_weapon_damage_multiplier`（`equipment.rs:133`）、`roll_equipment_drops`（`equipment.rs:199-215`）、`merge_equipment`（`equipment.rs:568`）；spawn 点 `spawn/rogue.rs` / `spawn/disciple.rs:145-157` / `heiwushi_spawn.rs`；realm 入参依赖 [[plan-npc-realm-distribution-v1]] P0（境界正确才配得对装备档）
- **出料**：攻方伤害进 `combat/resolve.rs:526-552`（现 NPC 恒 base=1.0 赤手）；防方减伤进 `DerivedAttrs.defense_profile` → `apply_armor_mitigation`；视觉进 valence `Equipment` component（client 免费渲染）；掉落 append 进 `npc/loot.rs` 的 `roll_loot` 结果
- **共享类型**：武器语义对齐玩家侧 `weapon_spec`（reach / wound_kind / damage_multiplier 语义一致）；护甲经 `ArmorProfileRegistry` 同一套 profile
- **跨仓库契约**：无 wire 变更（valence Equipment packet 既有；掉落物走既有 dropped_loot 链）
- **worldview 锚点**：§四:215 战力分层（体表层 = 兵刃与甲）；§七:739 智能 NPC 散修（末法散修靠凡铁兵刃讨生活，赤手木桩违背基调）；§一 末法（装备破旧、战损耐久是叙事）
- **qi_physics 锚点**：不涉及真元流动；灵兵/法器若带 qi 语义归 forge 既有系统，本 plan 只接凡兵凡甲

## 背景调研结论（2026-07-03）

- `assign_npc_equipment` 生产调用者 = **0**；`NpcEquipment` insert 点 = **0**（只在自身/combat_power/metadata 及测试出现）。对比同 plan 的 P1 功法/交易在 `disciple.rs:145-157` 都接了，唯独漏装备
- 战斗恒赤手：resolve 读 `Weapon` component（仅玩家由 `PlayerInventory` 同步），NPC 无 → base 1.0/mult 1.0；护甲同理 `defense_profile` 恒空
- 黑武士"手里的剑" = `faction_tint.rs:43-47` 的 rank 标记物，非真武器；`merge_equipment` 从未被调
- 死亡掉落走静态 archetype 表（`loot.rs:63`），与身上装备零联动；`roll_equipment_drops`（每非空槽 30% + 战损耐久）写好未接
- `NpcDormantSnapshot`（`dormant/mod.rs:274-329`）无装备字段——接上线不做 P4，睡一觉装备就丢
- `compute_combat_power` 形参收 `Option<&NpcEquipment>` 但实参恒 None

## P0 spawn insert ⬜

- rogue / disciple / commoner(按 archetype 判空) / heiwushi / faction 首领 spawn 处 `insert(assign_npc_equipment(archetype, realm, faction, char_id))`；Beast/Zombie/SkullFiend/Fuya/DyingElder 维持空装（函数已返回空）
- 测试抓手：各 archetype spawn 后 `NpcEquipment` 存在性与档位 pin；同 char_id 两次 spawn 配装一致（确定性）；`compute_combat_power` 实参接上后战力含 quality 分

## P1 战斗接线 ⬜

- 攻方：`resolve.rs:526` `Weapon` 缺失时回退读攻方 `NpcEquipment.main_hand` → `npc_weapon_damage_multiplier` + `reach_for_weapon_kind` + `wound_kind_for_weapon`（三函数现成）
- 防方：新系统把 `NpcEquipment.armor_slots()` 经 `ArmorProfileRegistry` 聚合进 `DerivedAttrs.defense_profile`（模式照抄 `armor_sync.rs:13-90`，或扩其 query 兼容双源）
- 测试抓手：持刀 rogue vs 赤手 rogue 伤害差 pin；穿甲 NPC 受击减伤走 `apply_armor_mitigation` 分支；武器 reach 影响 `AttackIntent.reach`

## P2 视觉接线 ⬜

- `sync_npc_visual_profiles_system`（`faction_tint.rs:66-115`）改为：有 `NpcEquipment` 时调 `merge_equipment` 用真实武器/护甲覆盖 rank 标记物；无装备者维持现状
- 黑武士手持真剑、着甲散修可视——client 零改动（valence Equipment packet）
- 验收：进游戏目视 + `Equipment` component 内容 pin 测试

## P3 掉落联动 + 战损耐久 ⬜

- NPC 死亡 loot 处理 append `roll_equipment_drops(&eq, seed)`，用 `weapon_drop_durability` / `armor_drop_durability` 设生成物耐久（战损叙事：从死人身上扒的兵刃不满耐久）
- 测试抓手：固定 seed 掉落集 pin；掉落物 durability < 1.0；静态 loot 表与装备掉落并存不互斥

## P4 dormant 快照持久化 ⬜

- `NpcEquipment`/`NpcEquipSlot` 补 `Serialize/Deserialize`；`NpcDormantSnapshot` 加 `#[serde(default)] equipment: Option<NpcEquipment>`；dehydrate/hydrate 往返读写
- 测试抓手：往返 roundtrip pin；旧版快照（无字段）hydrate 不炸（serde default）；离屏战死掉落含装备（与 `release_dormant_qi_to_zone` 同处结算）

## P5 决策门：类玩家背包（B 组） ⬜

> 用户原话方向：「玩家能装备的 NPC 都要能装备，需要有和玩家一样的背包系统」。这与 v1 的刻意设计（`equipment.rs:1-9`：NPC 不走背包，6 槽静态、无容器无重量）直接冲突，是**架构级决策**，本 plan 只立门不实施：

- **路线 A（NpcEquipment 扩展）**：补 ExtraHand0/1 对齐玩家 8 槽 + worn 分层；仍无背包容器。成本低，但"捡起地上的箱子背走"做不到
- **路线 B（NPC 挂 PlayerInventory）**：天然复用武器同步/armor_sync/容器/重量/worn-held 全套；代价是几十个假设"实体是玩家"的系统要解耦（snapshot emit / move handler / freshness / forge …），性能面（数百 dormant NPC × 全量 inventory）需评估
- 后续独立 plan：AI 拾取/比较/换装 big-brain action（无论 A/B 都需要）
- 本相仅交付：两路线的 spike 调研报告 + 拍板记录进 §8.1

## §8 开放问题（升 active / P0 决策门前收口）

1. **B 组路线拍板**（P5）：A 扩槽 vs B 共享 PlayerInventory——用户决策，附性能预估后再拍
2. **v1 归档回标**：`finished_plans/plan-npc-combat-gear-v1.md` 文档与代码不符（P0 接线缺失），⚠️ 回标由人工做还是 v2 升 active 时顺带（一 PR 一 plan 约束）
3. **配装质量随 zone 缩放**：`assign_npc_equipment` 现按 archetype/realm/faction，是否叠 zone 灵气档（资源区散修装备更好）——与 [[plan-npc-realm-distribution-v1]] P1 同一张 zone 分档表
4. **掉落率与经济平衡**：每槽 30% 掉落 × 境界分布修复后的高境界 NPC = 玩家白嫖好装备的通道，数值需与骨币经济合账
5. **heiwushi 特例**：黑武士剑道叙事是否要专属配装表（现走通用 assign 会不会拿到不合叙事的凡刀）
