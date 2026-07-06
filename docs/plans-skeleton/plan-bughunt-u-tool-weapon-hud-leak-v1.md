# plan-bughunt-u-tool-weapon-hud-leak-v1（骨架）

> **骨架（草案）**。一句话主题：`server/src/network/weapon_equipped_emit.rs` 为了修“工具手持无模型”，把 `category=tool` 也包装成 `weapon_equipped.weapon_kind="tool"` 下发；但 client `WeaponEquippedHandler` 对非盾 payload 一律写入 `WeaponEquippedStore`，`WeaponHotbarHudPlanner` 又把该 store 无条件当成战斗武器槽来画。结果是：**采集工具/锄头会在战斗 HUD 侧槽里以紫框武器槽 + `?` 字形 + 耐久条出现；副手工具还会提前吃掉 off_hand 分支，让 trigger 法宝 HUD 不再显示**。这不是纯 UI 瑕疵，而是“手持模型专用 payload 泄漏进 combat HUD 语义层”的收口缺口。

> 立项动机：候选位于 `armor/combat/client-weapon-hud` 主链，玩家高频可达，且现有代码注释已经把契约写死：`tool_item_to_view` 明说“**仅用于手持 3D 模型渲染（HUD 不消费 tool 的耐久）**”；`plan-tools-v1` 也写明“**凡器不入 hotbar**”。当前 main 上 client HUD 仍直接消费它，属于高置信、对实际游玩体验有明确影响的 bug。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | tool render payload 泄漏进 combat weapon HUD | fix_pr | ⬜ |

## P0 — tool render payload 泄漏进 combat weapon HUD

- **现象链**：`server/src/network/weapon_equipped_emit.rs:71-80` 把 `ItemCategory::Tool` 构造成 `WeaponViewV1 { weapon_kind: "tool" }`，注释明确写“仅用于手持 3D 模型渲染（HUD 不消费 tool 的耐久）”；但 client `WeaponEquippedHandler` 对 `off_hand shield` 之外的 payload 没有 `tool` 分支，直接落入 `WeaponEquippedStore.putOrClear(...)`（`client/src/main/java/com/bong/client/network/WeaponEquippedHandler.java:54-64`）。随后 `WeaponHotbarHudPlanner.buildCommands()` 只要 `main_hand/off_hand` 非空就调用 `drawWeaponSlot()`（`client/.../hud/WeaponHotbarHudPlanner.java:54-63`），并继续用 `kindToGlyph()` 渲染字形、用 `durabilityRatio()` 画耐久条（`:105-116`）。
- **为什么这是 bug，不是设计**：
  - `docs/finished_plans/plan-tools-v1.md:55` 已写死“**不入 hotbar**：凡器走 inventory 主手装备槽；hotbar 仅放消耗品 / 技能卷轴”。
  - `WeaponHotbarHudPlanner.kindToGlyph()` 只支持 `sword/saber/staff/fist/spear/dagger/bow`，`tool` 会走默认分支直接显示 `?`（`client/.../hud/WeaponHotbarHudPlanner.java:185-196`）。这说明 HUD 侧从未把 `tool` 当成合法 combat weapon kind 设计过。
  - 现有测试只覆盖武器 / 盾 / trigger treasure（`client/src/test/java/com/bong/client/hud/WeaponHotbarHudPlannerShieldTest.java`、`...TreasureTriggerTest.java`），没有任何 `tool` HUD 契约，和 server 端“只为手持模型止血”形成明显断层。
- **对实际游玩体验的影响**：
  - 玩家主手或副手拿斧/镐/锄/采药刀时，HUD 左右武器侧槽会稳定出现**紫色武器框 + `?` 字形 + 武器式耐久条**，把凡器工具伪装成战斗武器；采集/挖矿/耕种时战斗 HUD 反馈被噪音污染。
  - 更糟的是，`InventoryEquipRules` 允许 `tool/hoe` 进副手（`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:134-140`），而 `WeaponHotbarHudPlanner` 只要看到 `off_hand` 有 `EquippedWeapon` 就不会再进入 shield / treasure fallback 分支（`client/.../hud/WeaponHotbarHudPlanner.java:59-79`）。因此**副手拿工具时，trigger 位激活法宝 HUD 会被工具侧槽吞掉**，玩家在真实有法宝激活态时反而失去那条反馈。
- **建议修复方向**：优先二选一并统一语义。A) `WeaponEquippedHandler` 或 `WeaponHotbarHudPlanner` 明确过滤 `weapon_kind="tool"`，让 `tool` 只服务 `HeldItemStackResolver` / 手持模型链；B) 若坚持让工具占 side slot，则必须补独立 `tool` 视觉语义（非武器边框、非 `?` 字形、且不吞 trigger treasure），同时把 `plan-tools-v1` / emit 注释一起改成新契约。当前更像应走 A。
- **验收抓手**：至少补 4 组 pin。1) `weapon_kind="tool"` 到达 client 后不应生成武器侧槽命令。2) 主手工具仍保留手持 3D 模型。3) 副手工具 + trigger treasure 并存时，HUD 仍能显示 trigger treasure。4) 真武器/盾/法宝既有 HUD 契约不回归。

## 反方裁决摘要

1. **Round 1 反方主张**：“把 tool 写进 `WeaponEquippedStore` 也许是有意复用 weapon HUD，让工具沿用侧槽耐久条。”裁决：不成立。server 发包注释已经限定“仅用于手持 3D 模型渲染，HUD 不消费 tool 耐久”，而 client `kindToGlyph()` 对 `tool` 只会给 `?`，说明 HUD 根本没有对应设计语义。
2. **Round 2 反方主张**：“即便 side slot 出现 `?`，也只是轻微视觉问题，不算真实玩法影响。”裁决：不成立。副手 `tool/hoe` 是合法装备路径，且当前 `off_hand` 工具会短路掉 `trigger treasure` fallback；这会让玩家在法宝真实激活时丢失 HUD 提示，已经越过‘纯 cosmetic’边界，进入战斗/资源使用反馈缺失。

## 开放问题

1. `tool` 过滤应放在 `WeaponEquippedHandler`（不污染 store）还是 `WeaponHotbarHudPlanner`（仅修 HUD 读侧）？前者语义更干净，但要确认不会误伤别的 client 消费者。
2. 若后续真的想给凡器工具做 side-slot HUD，是否应另开 `ToolHotbarHudPlanner` / 独立 store，而不是继续复用 `WeaponEquippedStore` 的 combat 语义？

## 审计来源

bughunt 线程 U，限定 `armor/combat/client-weapon-hud` 主路径的 report-only 骨架轮。候选由主代理人工复核，并按“两轮默认怀疑”逐项证伪后保留。当前结论是：**先立 skeleton plan，后续 fix PR 再决定过滤点与回归测试面**。
