# plan-zhenfa-trap-client-equip-gate-v1

> 主题：`warning_trap` / `blast_trap` / `slow_trap` 在正常 craft 链路里可产出，但 client `InventoryEquipRules` 不把它们识别为 tool，导致**手槽装不上；server 又禁止 tool 进 hotbar**，玩家做出来后实际无法按预期使用。
>
> 验证状态：2026-07-04 bughunt 线程 H 读码确认；已做两轮默认怀疑式证伪，未找到 client 侧绕过手槽装备的合法路径。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | client tool 识别口径与 server 对齐 | ⬜ |
| P1 | 装备/快捷栏/提示文案回归 | ⬜ |
| P2 | zhenfa 陷阱端到端可用性回归 | ⬜ |

## 读码证据

- `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java`
  - `TOOL_TEMPLATE_IDS` 仅含旧工具与采矿/伐木工具；**不含** `warning_trap` / `blast_trap` / `slow_trap`
  - `canEquip()` 主手/副手放行条件依赖 `isTool(itemId)`
  - `canPlaceIntoHotbar()` 也依赖 `isTool(itemId)` 判断客户端能否放进 hotbar
- `client/src/main/java/com/bong/client/inventory/InspectScreen.java`
  - 拖到装备槽走 `isEquipSlotDropValid() -> InventoryEquipRules.canEquip()`
  - `quickEquipFromGrid()` 先过 `InventoryEquipRules.isTool(item)`，未识别为 tool 时直接 return
  - quick-use 仅发 `quick_slot_bind` 绑定，不替代世界交互的实际持物语义
- `server/src/inventory/mod.rs`
  - `validate_move_semantics()` 明确 `ItemCategory::Tool` **禁止进 hotbar**
  - `validate_equip_to()` 明确 `ItemCategory::Tool` **允许进 main/off/extra hand**
- `server/src/craft/mod.rs`
  - `register_zhenfa_content_recipes()` 正常注册 `warning_trap` / `blast_trap` / `slow_trap` 配方与产物
- `server/assets/items/zhenfa.toml`
  - 三个物品在 item registry 里都声明为 `category = "tool"`

## 玩家影响

- 玩家按正常玩法做出这三种凡阵陷阱后，会遇到 client 装备入口和 server 权威校验背离：
  - 拖到手槽被 UI 拒绝
  - 尝试拖到 hotbar 即使 client 乐观放行，server 也会回 `forbidden_in_hotbar`
- 结果不是“手感差”而是**内容不可用**：陷阱符产出后无法稳定进入实际布阵/放置链路。

## P0 client tool 识别口径与 server 对齐 ⬜

- 收口 `InventoryEquipRules` 的 tool 识别来源：
  - 至少覆盖 `warning_trap` / `blast_trap` / `slow_trap`
  - 明确是否一并补齐同类 `ItemCategory::Tool` 漏项，避免继续靠散落白名单追债
- 验收抓手：
  - `InventoryEquipRules.canEquip()` 对三种陷阱主手放行
  - `preferredWeaponQuickEquipSlot()` 能为三种陷阱选出手槽

## P1 装备/快捷栏/提示文案回归 ⬜

- 校准 client 行为一致性：
  - hotbar/quick-use 高亮与落点判定不再给出误导性绿灯
  - 被 server 拒绝时的 toast 文案与真实规则一致
- 验收抓手：
  - `InspectScreen` 拖拽到装备槽成功
  - `InspectScreen` 不再把三种陷阱当普通可进 hotbar 的 1×1 杂物

## P2 zhenfa 陷阱端到端可用性回归 ⬜

- 端到端锁定正常游玩链：
  - craft 产出 `warning_trap` / `blast_trap` / `slow_trap`
  - 从背包装备到手槽
  - 右键世界交互链正确命中对应 zhenfa request
- 验收抓手：
  - client 单测：`InventoryEquipRules` / `InspectScreen` 回归
  - 必要时补 server 侧 pin，锁住 `ItemCategory::Tool => hand yes / hotbar no` 契约

## 开放问题

1. 只补这三种陷阱，还是把所有真实 `ItemCategory::Tool` 统一改成非白名单/共享真相源？
2. `array_flag` / `niche_house_puppet` 等同类 tool 漏项是否并入同一 PR，还是作为 follow-up 另立条目？
