# plan-bughunt-u-tool-weapon-hud-leak-v1

> **Active BugFix plan（由 skeleton 于 2026-07-11 升格）**。一句话主题：凡器工具为手持 3D 模型复用 `WeaponEquippedStore`，却被 `WeaponHotbarHudPlanner` 当成战斗武器槽渲染；本 plan 只修 client HUD 消费边界，不改 server wire、装备规则、模型注册或资产。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 第一性原理 RED：证明工具进入 HUD 且副手工具遮蔽 trigger 法宝 | ✅ 2026-07-11 |
| P1 | 最小修复：HUD 过滤 `weapon_kind="tool"`，保留手持模型链 | ✅ 2026-07-11 |
| P2 | JDK 17 完整门禁、主线同步与验收 | ✅ 2026-07-11 |

## 接入面与范围

- **进料**：`WeaponEquippedHandler` 把 `weapon_equipped.weapon_kind="tool"` 写入 `WeaponEquippedStore`；这是 `HeldItemStackResolver` 渲染凡器主手/副手 3D 模型所需的既有链路。
- **出料**：`WeaponHotbarHudPlanner.buildCommands()` 只把战斗武器、盾牌与法宝转成 `HudRenderCommand`；工具在战斗 HUD 语义上透明。
- **共享类型**：复用 `EquippedWeapon`、`WeaponEquippedStore`、`TreasureEquippedStore`、`HudRenderCommand`，不新建 store 或协议类型。
- **跨仓库契约**：无 wire 变更。server 继续下发 `weapon_kind="tool"`，client handler 继续保存，只有 HUD planner 拒绝把该 kind 当战斗武器。
- **正典/既有设计锚点**：`docs/finished_plans/plan-tools-v1.md:55` 明确“凡器不入 hotbar”；本修复不涉及真元、技能、动画、VFX、SFX 或视觉资产生成。
- **禁止扩 scope**：不改 `WeaponEquippedHandler` 的工具入 store 行为，不改 `InventoryEquipRules`，不改 server，不新增工具专属 HUD，不改依赖与生产配置。

## 已决方案

过滤点固定在 `client/src/main/java/com/bong/client/hud/WeaponHotbarHudPlanner.java`：

1. `WeaponEquippedStore` 是手持模型共享状态，不能在 handler 层丢弃工具，否则会回归“工具手持无模型”。
2. planner 在主手/副手分支把 `weaponKind="tool"` 视为“不占战斗武器槽”。
3. 副手工具不占槽后，继续执行既有盾牌 → off-hand 法宝 → 首个 trigger 法宝 fallback；不改变这些既有优先级。
4. 未知的非 `tool` kind 维持当前 `?` 兜底，本 plan 不把协议健壮性问题扩大为全量 kind 白名单重构。

## P0 — 第一性原理 RED 证真

在 `client/src/test/java/com/bong/client/hud/WeaponHotbarHudPlannerTreasureTriggerTest.java` 增加修复前可失败的契约测试：

- 主手只有 `weapon_kind="tool"` 时，不得生成紫框武器侧槽或 `?` glyph。
- 副手 `weapon_kind="tool"` 与 trigger 法宝并存时，必须显示法宝 `宝` glyph，而不是工具 `?` glyph。
- RED 必须在未改生产代码的 HEAD 上以 JDK 17 运行并记录失败断言，证明真实断点位于 planner 消费边界。

## P1 — 最小修复与回归矩阵

- 在 `WeaponHotbarHudPlanner` 增加局部、可复用的 HUD 可见性判定，使 `tool` 不进入 `drawWeaponSlot()`。
- 饱和回归覆盖：
  - 主手工具隐藏；
  - 副手工具隐藏且不遮蔽 trigger 法宝；
  - 真武器仍渲染；
  - off-hand 盾牌优先级不变；
  - off-hand 法宝与 trigger 法宝既有 fallback 不变；
  - `HeldItemStackResolverTest` 继续证明工具仍可从 `WeaponEquippedStore` 解析为手持模型。
- 生产改动限定为 client Java；不修改资源、schema、server 或 agent。

## P2 — 验收与门禁

- 针对性测试：JDK 17 下运行 `WeaponHotbarHudPlannerTreasureTriggerTest`、`WeaponHotbarHudPlannerShieldTest`、`HeldItemStackResolverTest`。
- 完整 client gate：`cd client && ./gradlew test build`，JDK 必须为 17。
- 编译前检查全局 `cargo` / `gradle` / `rustc` 进程；已有 2 条编译 worktree 时等待，不清共享 target。
- 同步最新 `origin/main` 后，如 HEAD 或相关代码变化，重跑完整 client gate 与绑定新 HEAD 的只读 validator。

## 验收标准

1. 玩家主手/副手拿凡器时，战斗 HUD 不出现紫框、`?` glyph 或武器式耐久条。
2. 副手工具与 trigger 法宝并存时，法宝 HUD 正常显示。
3. 工具手持 3D 模型链不回归，真武器/盾牌/法宝 HUD 行为不变。
4. JDK 17 `./gradlew test build` 全绿，最终 HEAD 获得无上下文 read-only validator 的 `PASS <sha>`。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/hud/WeaponHotbarHudPlanner.java`：新增 `isHudWeapon(EquippedWeapon)`，主手/副手仅过滤 `weapon_kind="tool"`；副手工具继续进入既有盾牌/法宝 fallback。
- `client/src/main/java/com/bong/client/inventory/InspectScreenBootstrap.java`：断线装备快照清场补齐 `EquippedShieldStore`，避免旧 session 盾牌在新 session 工具/空手 fallback 时重返战斗 HUD。
- `client/src/main/java/com/bong/client/network/WeaponEquippedHandler.java`：写 store 前严格校验 slot 与 weapon JSON 类型，畸形/迟到包不能伪装成卸下并清空权威状态。
- `client/src/test/java/com/bong/client/hud/WeaponHotbarHudPlannerTreasureTriggerTest.java`：新增主手工具隐藏与副手工具不遮蔽 trigger 法宝两条契约测试。
- 未修改 `WeaponEquippedHandler`、`WeaponEquippedStore`、`HeldItemStackResolver`、server wire、schema、资源或依赖。

### 关键 commit

- `8a4ac598`（2026-07-11）：升格 skeleton，收口 client-only 修复范围与测试矩阵。
- `30abaa9a`（2026-07-11）：提交修复前 RED 契约；JDK 17 精确测试 7 项中新增 2 项按预期失败。
- `848f78b2`（2026-07-11）：在 HUD consumer 最小过滤工具，保留共享手持模型状态。
- `88bac38c`（2026-07-12）：补齐盾牌跨 session 清理，并锁住全武器 kind、代表性工具、切换/卸下和拒绝 payload 生命周期。
- `c10e2865`（2026-07-12）：修复 validator 指出的畸形装备包清场漏洞，覆盖所有非对象 JSON 类型及非法 slot。
- `6e3538fd`（2026-07-12）：校准 proto roundtrip fixture 的合法装备 slot，锁住跨端正向链路。

### 测试结果

- RED：`./gradlew test --tests com.bong.client.hud.WeaponHotbarHudPlannerTreasureTriggerTest`，JDK 17.0.19；7 tests，2 failed（两条新增契约，符合修复前预期）。
- 针对性 GREEN：`WeaponHotbarHudPlannerTreasureTriggerTest`、`WeaponHotbarHudPlannerShieldTest`、`HeldItemStackResolverTest` 同批通过。
- closeout 定向 gate：JDK 17.0.19 下运行 planner、装备 handler、断线清理、手持模型与破盾旧实例测试；80 tests / 0 failures / 0 errors。
- closeout 完整 client gate：同步最新主线前为 3761 tests；合并 `origin/main=f8b4ab112424db62a008c4fc17d20cf8f49c4b28` 后为 3925 tests；validator 返工后最终重跑为 3927 tests / 0 failures / 0 errors / 0 skipped，JDK 17.0.19，`BUILD SUCCESSFUL`。
- 主线同步：`origin/main=f8b4ab112424db62a008c4fc17d20cf8f49c4b28` 是最终修复 HEAD 祖先；自动 merge 无冲突，相关 client 完整门禁已在 merge 后重跑。
- closeout 第一轮无上下文只读 validator：`FAIL 9f681bd84bb0c388a42bb613a71c1c9e7deb5e56`，发现非对象 `weapon` / 非法 `slot` 可误清权威状态；已由 `c10e2865` 修复并补饱和测试。

### 跨仓库核验

- client：`WeaponEquippedStore` 继续接收工具供 `HeldItemStackResolver` 使用；`WeaponHotbarHudPlanner` 不再消费工具为战斗槽；断线统一清理武器、盾牌与法宝 HUD fallback。
- server：`weapon_equipped.weapon_kind="tool"` 契约保持不变，无 server diff。
- agent/schema/worldgen：本 plan 无接入面，无 diff、无需跨栈构建。

### 遗留 / 后续

- 未知的非 `tool` `weapon_kind` 继续显示 `?`，保持既有兼容行为；若未来需要严格 kind 白名单，应另立独立协议健壮性任务。
- 本 plan 不新增凡器专属 HUD；若产品未来需要工具耐久 UI，应使用独立 planner/store 语义，不复用战斗武器槽。
