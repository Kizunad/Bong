# plan-bughunt-tsy-death-extra-hand-protection-v1

> 骨架：来源 #1876。TSY 死亡掉落只保护 main/off hand，`iter_all` 仍把 extra hand 真武器加入候选。

## §0 摘要

`server/src/inventory/tsy_death_drop.rs:63-82` 的 `protected_weapon_ids` 只收集 `EQUIP_SLOT_MAIN_HAND/OFF_HAND`；`:117-130` 遍历所有 equipped contents。extra hand 中耐久足够的 weapon 不在保护集合，进入 entry-carry 50% roll，与主世界规则不一致。

## §1 游玩影响

玩家在额外手槽装备的关键武器会在 TSY 死亡时被错误掉落，死亡风险高于同样装备在主世界的风险。

## §2 复现路径

1. 将耐久 ≥ 0.5 的真武器放入 extra hand。
2. 在 TSY 内死亡并走 entry-carry drop。
3. 观察 weapon instance 未被 `protected_weapon_ids` 排除，进入随机掉落候选。

## §3 今天 `origin/main` 根因证据

- `tsy_death_drop.rs:63-82` 保护集合硬编码两种手槽。
- `:117-130` 对所有 equipped 槽收集 entry/TSY acquired，extra hand 无额外保护判断。
- 注释声称与主世界规则一致，但实际槽位集合不一致。

## §4 非重复比对

`plan-bughunt-tsy-death-drop-anchor-v1` 处理掉落坐标，普通 inventory death-drop 规则已存在；本骨架只统一 TSY extra hand 的 weapon protection。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“TSY 死亡、武器、手持保护”；`worldview.md §十二` 规定秘境死亡规则与主世界持握保护一致。
- **finished_plans**：查 `protected_weapon_ids`、`iter_all`、extra hand；未见槽位扩展契约。
- **active plan**：查 tsy loot/death drop；无同一保护修复。
- **skeleton**：查 `extra hand`、`tsy death drop`、`protected weapon`；无重复骨架。
- **reminder.md**：查 `TSY`、`extra_hand`、`weapon`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`PlayerInventory.equipped`、`ItemRegistry.weapon_spec`、durability、`TsyPresence`。
- **Outputs**：保护/掉落 instance 集合、`TsyDeathDropOutcome`、dropped registry。
- **共享类型或事件**：复用 `EquipmentContents::iter_all`、`weapon_spec`、`TsyDeathDropOutcome`；保护判定应与主世界 helper 共用。
- **server 符号**：`inventory::tsy_death_drop::apply_tsy_death_drop`、`apply_death_drop_to_inventory`、`ItemRegistry`。
- **agent**：无变更；死亡掉落不经 agent。
- **client**：无变更；既有 inventory/drop payload。
- **worldview 锚点**：`docs/worldview.md §十二` TSY 死亡与持握武器保护。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 复用统一 weapon protection helper 覆盖 extra hand，保持 entry/TSY 分类语义 |
| P1 | ⬜ | 主手、副手、双手、extra hand、低耐久和重复死亡回归测试 |

## 来源 issue

- #1876 `[flash-review][major] TSY 死亡掉落未保护 extra_hand 手持武器`
