# plan-npc-combat-gear-v2 — NPC 装备复活：v1 孤儿代码接线补全 + 类玩家背包决策门

> **骨架**（2026-07-03）。一句话：`plan-npc-combat-gear-v1` 的 `NpcEquipment` 整套模型（配装/掉落/视觉合并）已完成却**从未被任何 spawn 路径 insert、战斗结算不读、掉落不联动、快照不持久**——v2 先把这五段断线全部接活（NPC 手持真武器、穿甲真减伤、死亡掉真装备），再以决策门形式收"NPC 类玩家背包"的架构拍板。
>
> **为何开 v2 不并入 v1**：v1 已在 `docs/finished_plans/` 且自报 ✅，但其 P0 承诺的 spawn 接线与 `sync_npc_equipment_system` 实际缺失（文档与代码不符，属 ⚠️ 红旗）。v1 归档文档本 PR 不动（一 PR 一 plan），差异在本节登记，回标交人工或升 active 时处理。

> **§8.1 #1 已拍板（2026-07-03）：走 B 路线——NPC 挂真 `PlayerInventory`**。相位表按 B 重排；
> 原 A 路线（NpcEquipment 五段接线）保留在 §附录A 作回退预案，不实施。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | B 路线 spike：玩家假设解耦审计 + 性能预估（可行性门） | ⬜ |
| P1 | NPC 挂 PlayerInventory + 配装生成器（archetype/境界/派系 → equipped） | ⬜ |
| P2 | 战斗自动生效（weapon/armor 同步系统去"实体=玩家"假设） | ⬜ |
| P3 | 视觉同步（手持/护甲经 valence Equipment 对 NPC 实体生效） | ⬜ |
| P4 | 掉落 = 尸体整包搜刮 + 战损耐久 | ⬜ |
| P5 | dormant 快照持久化（inventory 字段往返） | ⬜ |

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

## P0 B 路线 spike（可行性门） ⬜

- 审计全部假设"持 `PlayerInventory` 的实体是玩家/Client"的系统（snapshot emit、move handler、freshness、forge、alchemy、material-discovery、重量、hotbar…），产出解耦清单：每系统标注 对NPC应生效（armor/weapon 同步）/ 应跳过（UI emit 类，query 加 `With<Client>` 收口）/ 需改造
- 性能预估：数百 dormant NPC × 全量 inventory 的内存与 tick 成本；`Changed<PlayerInventory>` 驱动的系统在 NPC 数量级下的扫描面
- **回退门**：spike 判定不可行（成本清单爆炸/性能不过）→ 回退 §附录A 的 A 路线，决议记录进 §8.1
- 交付：spike 报告落 plan 本文件 §9（升 active 时建）

## P1 NPC 挂 PlayerInventory + 配装生成器 ⬜

- 可持械 archetype（rogue/disciple/commoner/heiwushi/派系首领）spawn 时 insert `PlayerInventory`：equipped 按配装表生成真实 `ItemInstance`（走 `ItemRegistry` 模板 + `InventoryInstanceIdAllocator`，杜绝伪物品）
- 配装表：迁移 `assign_npc_equipment`（`equipment.rs:275-302`）的 archetype/realm/faction 确定性档位逻辑为"产出 equipped 内容"的生成器；`NpcEquipment` 组件与 `merge_equipment`/`roll_equipment_drops` 等孤儿代码**退役删除**（不留兼容层），数值表全量迁移
- main_pack 容器塞少量随身杂物（与 `NpcLootTable` 合流的机会点，§8 #4 数值合账）
- realm 入参依赖 [[plan-npc-realm-distribution-v1]] P0
- 测试抓手：各 archetype spawn 后 equipped 槽位/档位 pin；同 char_id 确定性；物品 instance_id 全局唯一

## P2 战斗自动生效 ⬜

- `sync_weapon_component_from_equipped`（`combat/weapon.rs:122-161`）与 `armor_sync.rs:13-90` 的 query 去掉玩家专属假设 → NPC 实体天然获得 `Weapon` component 与 `defense_profile`，`combat/resolve.rs` **零改动**正确读取
- UI 类 emit 系统（inventory snapshot 推送等）query 加 `With<Client>` 防对 NPC 空发包
- 测试抓手：持刀 NPC vs 赤手 NPC 伤害差 pin；穿甲 NPC 受击走 `apply_armor_mitigation`；NPC 不触发 inventory snapshot 网络包

## P3 视觉同步 ⬜

- 玩家侧手持/护甲 → valence `Equipment` 的同步路径对 NPC 实体生效（替换 `faction_tint.rs:43-47` 的 rank 标记物；黑武士手持真剑）
- client 零改动（既有 Equipment packet 渲染）
- 验收：进游戏目视 + `Equipment` component 内容 pin

## P4 掉落 = 尸体整包搜刮 + 战损耐久 ⬜

- B 路线红利：NPC 死亡不再走"掉落表 roll"，而是**尸体容器暴露其真实 `PlayerInventory`**（塔科夫式搜尸——复用 tarkov-backpack 的容器视图先例与 `ExternalContainer` 链），拿走什么就少什么，与 [[plan-mundane-fauna-v1]] 尸体窗口语义合流
- 装备件生成时即带战损耐久（<1.0，末法叙事）；`NpcLootTable` 静态表退化为"随身杂物填充源"并入 P1 的 main_pack
- 测试抓手：搜尸取件后尸体容器与世界物品守恒；耐久区间 pin

## P5 dormant 快照持久化 ⬜

- `NpcDormantSnapshot` 加 `#[serde(default)] inventory: Option<PlayerInventory>`（`PlayerInventory` 已 Serialize——bong.db 持久化在用）；dehydrate/hydrate 往返
- 离屏战死 → 尸体整包与 `release_dormant_qi_to_zone` 同处结算
- 测试抓手：往返 roundtrip；旧版快照（无字段）hydrate 不炸；离屏死亡不吞装备

## 后续独立 plan（本 plan 不做）

- AI 拾取/比较/换装 big-brain action（NPC 捡地上的好装备穿上）
- NPC 用容器类装备（背包套娃）的 AI 语义

## §8 开放问题（升 active / P0 决策门前收口）

> #1 已在 §8.1 收口（用户拍板 2026-07-03：B 路线）；其余升 active 前收口，#2 v1 回标待人工。


1. **B 组路线拍板**（P5）：A 扩槽 vs B 共享 PlayerInventory——用户决策，附性能预估后再拍
2. **v1 归档回标**：`finished_plans/plan-npc-combat-gear-v1.md` 文档与代码不符（P0 接线缺失），⚠️ 回标由人工做还是 v2 升 active 时顺带（一 PR 一 plan 约束）
3. **配装质量随 zone 缩放**：`assign_npc_equipment` 现按 archetype/realm/faction，是否叠 zone 灵气档（资源区散修装备更好）——与 [[plan-npc-realm-distribution-v1]] P1 同一张 zone 分档表
4. **掉落率与经济平衡**：每槽 30% 掉落 × 境界分布修复后的高境界 NPC = 玩家白嫖好装备的通道，数值需与骨币经济合账
5. **heiwushi 特例**：黑武士剑道叙事是否要专属配装表（现走通用 assign 会不会拿到不合叙事的凡刀）

## §8.1 决议（用户拍板 2026-07-03）

### #1 B 组路线：**B——NPC 挂真 `PlayerInventory`**

1. 用户拍板走 B：玩家能装备的 NPC 都能装备、能背包携带；掉落顺势升级为尸体整包搜刮（P4）
2. `NpcEquipment` 整套孤儿代码退役删除（不留兼容层，符合仓库"干净代码"约定）；其配装数值表迁移进 P1 配装生成器
3. P0 spike 保留**回退门**：性能/解耦成本爆炸时回退附录 A 的接线路线，届时在本节追记

## 附录 A（回退预案，不实施）：NpcEquipment 五段接线

原 v2 P0-P4 方案备份：spawn insert `assign_npc_equipment` → resolve 读 `NpcEquipment.main_hand` 回退分支 + armor 聚合 → `merge_equipment` 视觉 → `roll_equipment_drops` 掉落 → 快照加 equipment 字段。体量小-中、全复用孤儿代码，但 6 槽静态无背包，与用户"类玩家背包"预期冲突，仅作 P0 spike 失败时的回退预案。
