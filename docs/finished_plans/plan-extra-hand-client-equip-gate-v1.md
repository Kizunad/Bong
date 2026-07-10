# plan-extra-hand-client-equip-gate-v1

> 主题：client `InventoryEquipRules` / `EquipmentPanel` 把 `EXTRA_HAND_0/1` 当成 `OFF_HAND` 处理，导致多臂额外手槽只能装匕首/拳/工具/锄头，并且在主手持双手武器时被错误锁死；server 与既有 plan 明确 `extra_hand` 是**独立 held 槽**，可装一般武器、且**不受主/副手双手锁影响**。结果不是“手感差”，而是**化虚期丹道阶段 4 的多臂核心收益在真实 UI 上大半失效**：玩家解锁额外两手后，剑/刀/弓/杖等正常武器拖不上去，主手拿双手杖时空闲多臂槽还会被直接灰掉。
>
> 验证状态：2026-07-05 bughunt 线程 AN 读码确认，已做两轮默认怀疑式证伪。
> 1. 反方一：“也许 extra hand 本来就该复用副手规则。”证伪：`docs/finished_plans/plan-layered-equip-v1.md:186-187` 明写 `main_hand / off_hand / extra_hand_0 / extra_hand_1` 都是 held 槽，且 **extra_hand 独立不锁**；`server/src/inventory/mod.rs:15061-15105` 还用 `test_sword` pin 住 `ExtraHand0/1` 可装一般武器。
> 2. 反方二：“也许只是显示层灰掉，server 仍可正常玩。”证伪：`InspectScreen` 的拖拽/shift-quick-equip 主路径都先过 client 门控；`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2234` 的 `quickEquipFromGrid(item)` 与 `:3584-3592` 的 `isEquipSlotDropValid()` 都依赖 `InventoryEquipRules.canEquip()`，因此这是**真正拦截玩家输入**，不是纯视觉差异。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | client extra hand 装备规则与 server 对齐 | ✅ 2026-07-06 |
| P1 | 多臂槽双手锁视觉/交互回归 | ✅ 2026-07-06 |
| P2 | 快装/拖拽/契约 pin 测试补齐 | ✅ 2026-07-06 |

## 接入面

- **进料**：背包格物品进入 `InspectScreen` 拖拽/shift 快装主路径。
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java:2234` `quickEquipFromGrid(item)`
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java:3584-3592` `isEquipSlotDropValid() -> InventoryEquipRules.canEquip()`
- **出料**：client 判定通过后才会构造 `EquipLoc(extra_hand_0|1, held)` 并发 `InventoryMoveIntent` 到 server。
  - `InspectScreen.java:3642-3646` 发 `dispatchMoveIntent(...)`
  - server `server/src/inventory/mod.rs:5098-5165` `validate_equip_to()` 权威校验
- **共享类型 / event**：
  - `client/.../inventory/model/EquipSlotType.java`：`EXTRA_HAND_0/1` 为 held 槽
  - `server/src/schema/inventory.rs` / proto 的 `EquipSlotV1::ExtraHand0/1`
- **跨仓库契约**：
  - server 已锁定 `ExtraHand0/1` 可装一般武器：`server/src/inventory/mod.rs:15061-15105`
  - server 已锁定主手双手武器**不锁** extra hand：`server/src/inventory/mod.rs:9545-9567`
  - client 当前却把 `EXTRA_HAND_0/1` 合并进 `OFF_HAND` 规则：`client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:127-143`
- **worldview 锚点**：
  - `docs/finished_plans/plan-dandao-path-v1.md:46-49`：阶段 4 多臂 = `副手 slot +2`，用于快速切换，不是同时四连击
- **qi_physics 锚点**：N/A。纯 client 装备门控/视觉状态错误，不涉及真元流转。

## 读码证据

- `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:127-143`
  - `MAIN_HAND` 分支把双手武器入主手的前置条件错误写成 “`OFF_HAND` + `EXTRA_HAND_0` + `EXTRA_HAND_1` 全空”
  - `OFF_HAND, EXTRA_HAND_0, EXTRA_HAND_1` 被并入同一分支，只放行 `dagger/fist/treasure/shield/tool/hoe`
  - 这意味着 `iron_sword` / `bronze_saber` / `wooden_staff` / `bow` 等一般武器**永远不可能**通过 client extra hand 装备门
- `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java:160-165`
  - 注释与实现都把 “主手双手武器” 的锁定面扩大成了“副手/多臂”
- `client/src/main/java/com/bong/client/inventory/component/EquipmentPanel.java:109-121`
  - `applyTwoHandLock()` 会把空闲 `OFF_HAND`、`EXTRA_HAND_0`、`EXTRA_HAND_1` 一起 `setDisabledByTwoHand(true)`
  - 即使 server 明确 extra hand 独立不锁，client 也会先把多臂槽灰掉，玩家甚至不会去尝试拖入
- `server/src/inventory/mod.rs:5102-5118`
  - server 只给 `OFF_HAND` 追加 `Treasure | Shield` 与 `dagger/fist` 限制
  - `ExtraHand0/1` 走的是“任意 weapon/tool/hoe”路径，不带 `OffHandTypeMismatch`
- `server/src/inventory/mod.rs:5133-5157`
  - 双手武器锁对侧手只在 `main_hand <-> off_hand` 之间计算，`extra_hand` 不参与
- `server/src/inventory/mod.rs:15061-15105`
  - `validate_move_semantics_accepts_weapon_to_extra_hand_0/1` 用 `test_sword` 直接 pin 住：一般武器可装 extra hand
- `server/src/inventory/mod.rs:9545-9567`
  - `validate_two_handed_main_hand_does_not_lock_extra_hand` 明确 pin：主手双手杖时，`extra_hand_0` 仍可装 `bone_dagger`
- `client/src/test/java/com/bong/client/inventory/InventoryEquipRulesTest.java:71-81`
  - 现有 client 测试还把**错误行为**锁死：主手双手武器时，断言 `EXTRA_HAND_0` 也必须被锁
- `client/src/test/java/com/bong/client/inventory/component/EquipmentPanelTest.java:136-149`
  - 现有 client 面板测试同样把“多臂槽应被双手锁灰掉”当成正确行为

## 玩家影响

- 这是阶段 4 多臂奖励的主玩法断链，不是边角案列。
  - 玩家解锁 `EXTRA_HAND_0/1` 后，想把第二把剑、备用弓、双手杖、长柄等塞进额外手槽做“无延迟切换”，client 会直接拒绝拖拽
  - 主手已经拿双手武器时，空闲多臂槽还会被灰掉，进一步误导玩家“这个槽不可用”
  - shift 快装同样坏掉：`quickEquipFromGrid()` 依赖同一套 `canEquip()` / `preferredWeaponQuickEquipSlot()`，主手占用后不会把一般武器路由到 extra hand
- 体感上表现为：**丹道阶段 4 的“多臂 +2 手槽”看得见、摸不着**。玩家辛苦打到高变异阶段，UI 却把最核心的持械收益封死，构成明确的实际游玩损害。

## P0 client extra hand 装备规则与 server 对齐 ✅ 2026-07-06

- 交付物：
  - `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java`
    - 把 `OFF_HAND` 与 `EXTRA_HAND_0/1` 分支拆开
    - `OFF_HAND` 继续维持当前语义：`dagger/fist/treasure/shield/tool/hoe`
    - `EXTRA_HAND_0/1` 改与 server 对齐：允许 `weaponKind != null || tool || hoe`；**不**继承 `treasure/shield` 的副手特权
    - `MAIN_HAND` 的双手前置只检查 `OFF_HAND` 是否空闲，不再要求 `EXTRA_HAND_0/1` 也空
    - `mainHandHoldsTwoHand` 或其调用点改成只服务 `OFF_HAND`，不再误伤 extra hand
- 验收抓手：
  - `iron_sword` / `bronze_saber` / `wooden_staff` / `bow` 可装 `EXTRA_HAND_0/1`
  - `starter_talisman` / `wooden_shield` 仍只能走 `OFF_HAND`，不可进 `EXTRA_HAND_0/1`
  - 主手双手杖时，`OFF_HAND` 被锁、`EXTRA_HAND_0/1` 不锁

## P1 多臂槽双手锁视觉/交互回归 ✅ 2026-07-06

- 交付物：
  - `client/src/main/java/com/bong/client/inventory/component/EquipmentPanel.java`
    - `applyTwoHandLock()` 只 disable 空闲 `OFF_HAND`
    - 不再把 `EXTRA_HAND_0/1` 灰掉
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java`
    - 代码路径本身应因 P0 自动恢复；本阶段重点是补 interaction pin，锁住“拖拽/shift 快装能走到 extra hand”
- 验收抓手：
  - 主手持 `wooden_staff` 时，装备面板中 `OFF_HAND` 灰显，但 `EXTRA_HAND_0/1` 仍可高亮落点
  - grid 里的 `iron_sword` 在主手已占、off_hand 不合法时，`quickEquipFromGrid()` 会把它路由到 `EXTRA_HAND_0/1`

## P2 快装/拖拽/契约 pin 测试补齐 ✅ 2026-07-06

- 交付物：
  - `client/src/test/java/com/bong/client/inventory/InventoryEquipRulesTest.java`
    - 改写现有错误断言：主手双手武器**不应**锁 extra hand
    - 新增/改写 pin：
      - 一般武器可装 `EXTRA_HAND_0/1`
      - 主手双手武器时，一般武器仍可装 extra hand
      - `preferredWeaponQuickEquipSlot()` 在主手已占、off_hand 不合法时，会把一般武器选到 extra hand
      - `shield/treasure` 不可装 extra hand
  - `client/src/test/java/com/bong/client/inventory/component/EquipmentPanelTest.java`
    - 改写现有错误断言：双手武器只锁 `OFF_HAND`
    - 补多臂槽仍可交互/不灰显的 panel 状态测试
  - `client/src/test/java/com/bong/client/inventory/InspectScreenMoveIntentTest.java`
    - 通过生产路径共用的最薄 headless 编排 seam 覆盖拖拽落入 `EXTRA_HAND_1`
    - 覆盖 shift 快装在主手占用、副手不合法时落入 `EXTRA_HAND_0`
    - 断言最终发送的 `InventoryMoveIntent` 携带正确 `EquipLoc`；无合法槽位/目标已占时不发请求
- 验收抓手：
  - client 单测能直接表达“server 接受的一般武器，client 也必须接受”
  - 不再把当前错误行为写进测试名称/断言文案

## §8 开放问题（已收口）

1. extra hand 是否应该允许 `shield/treasure`，还是严格跟 server 现状走“仅 weapon/tool/hoe”？
2. 是否需要改 server 跟随 client，统一把 extra hand 降级成 off-hand 语义？

## §8.1 决议（pre-P0 收口，2026-07-05）

### #1 extra hand 允许范围

**决议**：严格对齐 server，而不是继续扩散 client 私货规则。

- 依据：
  - server `validate_equip_to()`（`server/src/inventory/mod.rs:5102-5118`）明确只有 `OFF_HAND` 才有 `Treasure | Shield` 特权与 `dagger/fist` 限制
  - `ExtraHand0/1` 的 server pin 测试（`mod.rs:15061-15145`）只声明 “weapon/tool/hoe 可装”
- 落点：
  - client `InventoryEquipRules` extra hand 分支只接受 `weaponKind != null || tool || hoe`
  - `shield/treasure` 继续只允许 `OFF_HAND`

### #2 是否改 server 迁就 client

**决议**：不改 server，修 client。

- 依据：
  - `docs/finished_plans/plan-layered-equip-v1.md:186-187` 与 `docs/finished_plans/plan-dandao-path-v1.md:46-49` 已把 extra hand 的设计锚定为**独立额外持握槽**
  - server 已有明确 pin 测试和运行时语义，当前错误完全集中在 client
  - 若反向把 server 改成 off-hand 语义，等于把已承诺的阶段 4 多臂收益砍掉，不是 bugfix，是设计回退

## 反方裁决摘要

- 反方一主张“多臂只是 UI 装饰或后续能力，client 现在限制更多种类也合理”。裁决：驳回。server 与 finished plan 已明确把 extra hand 定义成真实 held 槽，并且有运行时/测试双重证据证明一般武器应可进入。
- 反方二主张“主手双手武器锁 extra hand 可能是为了避免四持 OP”。裁决：驳回。阶段 4 的限制是“快速切换，非同时攻击”，不是“额外手槽失效”；`plan-layered-equip-v1` 已明写 **extra_hand 独立不锁**，当前 client 是违背既有决议，不是平衡设计。

## Finish Evidence

- **落地清单**：
  - `client/src/main/java/com/bong/client/inventory/InventoryEquipRules.java`：拆分 `OFF_HAND` 与 `EXTRA_HAND_0/1`，extra hand 允许 `weapon/tool/hoe`，不继承 `shield/treasure` 副手特权；主手双手武器只检查 `OFF_HAND` 空闲。
  - `client/src/main/java/com/bong/client/inventory/component/EquipmentPanel.java`：双手武器只 disable 空闲 `OFF_HAND`，不再灰显 `EXTRA_HAND_0/1`。
  - `client/src/test/java/com/bong/client/inventory/InventoryEquipRulesTest.java`：锁住一般武器可进 extra hand、主手双手武器不锁 extra hand、quick-equip 路由 extra hand、shield/treasure 不进 extra hand。
  - `client/src/test/java/com/bong/client/inventory/component/EquipmentPanelTest.java`：锁住双手武器只灰副手、多臂槽仍可交互。
  - `client/src/main/java/com/bong/client/inventory/InspectScreen.java`：拖拽装备提交抽取为生产/headless 共用编排边界，shift 快装继续调用同一生产方法。
  - `client/src/test/java/com/bong/client/inventory/InspectScreenMoveIntentTest.java`：锁住拖拽与 shift 快装从输入编排到 extra-hand `EquipLoc`、`InventoryMoveIntent` 的真实接入链路。
- **关键 commit**：
  - `6921a924`（2026-07-06）：修复多臂额外手槽客户端装备门控。
  - `41b9a73c`（2026-07-06）：通过 PR #900 接入 `origin/main`。
- **测试结果**：
  - 2026-07-10 使用 JDK 17 运行 Gradle 定向测试：`InventoryEquipRulesTest` 43 条 + `EquipmentPanelTest` 11 条 + `InspectScreenMoveIntentTest` 17 条，71/71 PASS。
- **跨仓库核验**：
  - server 已有 `validate_move_semantics_accepts_weapon_to_extra_hand_0/1` 与 `validate_two_handed_main_hand_does_not_lock_extra_hand` pin；extra-hand 实现子集未改 server/schema。本 PR 是 31 项本地分叉的统一 reconciliation 容器，另含伪灵脉恢复与 social 回归等独立残余语义，不属于本 plan 的交付范围。
  - client `InspectScreen` 拖拽与 shift quick-equip 继续经 `InventoryEquipRules.canEquip()` / `preferredWeaponQuickEquipSlot()`，规则层已补 pin。
- **验证边界 / 后续**：headless 回归从 owo hit-test 后的生产编排边界开始，覆盖拖拽/Shift 快装、失败回源、`EquipLoc` 与 `InventoryMoveIntent`；当前自动化不声称覆盖 mounted owo 的像素坐标命中。`EXTRA_HAND_0/1` 实际坐标命中与 disabled 视觉回退保留为 `runClient` 人工 UI 回归项。
