# plan-bughunt-craft-zone-ledger — 制作真元移除固定 spawn 账户

## 0. 状态

- 分区：server-qi
- 状态：✅ 2026-07-14
- 类型：BugHunt 修复；按 `plan-zone-qi-economy-v1` 的共享待分配池架构收口
- 主题去重：已避开 #969-#1046 中 dormant / 灵物磨损 / heartbeat / 骨币 / 垂死大能 / 医道修复 / TSY 入场过滤 / NPC 技能 overflow / 骨煞抽取等守恒主题；相邻 craft PR 为 #1039 满包退款、#1030 outcome 网络线程、#1004 制作台跨维误拆，均不覆盖本题

## 1. 一句话 bug

生产 C2S 手搓路径把所有正 `qi_cost` 配方的 `QiTransferReason::Crafting` 目标 zone 固定写成 `zone:spawn`，玩家在青云残峰、灵泉湿地、坍缩渊等非 spawn 区域制作时，真元从玩家扣除却被记到错误区域账户。

## 2. 实际游玩体验影响

玩家远征到资源区后使用带真元成本的配方制作，会看到自己真元下降，但该区域不会在账本语义上获得这笔回流，反而让出生点 `spawn` 虚增 Crafting 入账。对末法残土玩法而言，这会让“在哪里消耗真元、哪里获得环境回流”的区域经济错位：资源区/负灵域的本地压力、后续审计、天道/生态判定都可能读到错误归因。

边界：这不是“吞真元”或全局总量不守恒；当前 `WorldQiAccount` 仍做 player -> zone 的零和转账。问题是目标 zone 错误，属于区域守恒账/玩家体验错位。

## 3. 正典与约束

- `worldview.md §一 L18-L20`：全服灵气总量不凭空产生，玩家修炼消耗的灵气就是别人少掉的灵气。
- `AGENTS.md §9`：所有真元/灵气流动必须走 `qi_physics::ledger::QiTransfer { from, to, amount, reason }`，区域归还不能落到错误账户。
- `docs/CLAUDE.md §四 L59`：`zone` 与玩家真元的增减必须对应，守恒不能只看全局总量，还要保证同源流动落在正确环境。

## 4. 根因证据

1. `server/src/network/craft_emit.rs:59-62` 定义：
   - 注释写明 inventory 手搓“暂时统一用 `spawn`”，后续才按 `Position -> ZoneRegistry` 解析真实 zone。
   - 常量为 `const DEFAULT_CRAFT_ZONE_ID: &str = "spawn";`
2. `server/src/network/craft_emit.rs:140-184` 的生产入口 `apply_craft_intents`：
   - 系统参数只有 `player_positions: Query<&Position>`，用于制作台距离检查。
   - 没有读取 `CurrentDimension` 或 `ZoneRegistry`。
   - 构造 `StartCraftRequest` 时固定 `zone_id: DEFAULT_CRAFT_ZONE_ID`。
3. `server/src/craft/session.rs:225-232` 已把 `zone_id` 设计成 caller 参数；底层没有硬编码。
4. `server/src/craft/session.rs:342-392`：
   - `to = QiAccountId::zone(request.zone_id)`。
   - `QiTransfer::new(from, to, total_qi_cost, QiTransferReason::Crafting)`。
   - 随后扣 `deps.cultivation.qi_current -= total_qi_cost`。
5. `server/src/world/zone.rs:298-323` 已提供正确 API：`find_zone(dim, pos)` / `find_zone_mut_by_pos(dim, pos)`，并明确要求调用方用 `CurrentDimension`，避免 TSY 维度硬编码。
6. C2S 输入不带 zone：
   - `server/src/schema/client_request.rs:654-662` 的 `CraftStart` 只有 recipe/quantity 语义。
   - `server/src/craft/events.rs:107-116` 的 `CraftStartIntent` 只有 caster/recipe_id/quantity。
   - 因此真实 zone 必须由 server 按玩家当前位置推导。
7. 影响真实正成本配方：
   - `server/src/craft/workbench_recipes.rs:2070-2097` pin 了 20 个 `qi_cost > 0` 的 workbench recipes。

## 5. 复现路径骨架

1. 准备 `ZoneRegistry`，至少包含 `spawn` 与 `lingquan_marsh`，并把玩家实体放在 `lingquan_marsh` AABB 内。
2. 玩家实体挂 `Position`、`CurrentDimension(DimensionKind::Overworld)`、`PlayerInventory`、`Cultivation`、`QiColor`，并把 `WorldQiAccount` 的 player 余额同步到 `cultivation.qi_current`。
3. 发 `CraftStartIntent { caster, recipe_id: <任一 qi_cost > 0 配方>, quantity: 1 }`。
4. 当前实际：`WorldQiAccount.balance(zone:spawn)` 增加 `qi_cost`，`zone:lingquan_marsh` 不增加。
5. 期望：`WorldQiAccount.balance(zone:lingquan_marsh)` 增加 `qi_cost`；若当前维度为 TSY，则解析到对应 TSY zone，找不到 zone 时才走明确 fallback / overflow 策略。

## 6. 修复结果

- [x] P0：移除 `StartCraftRequest.zone_id` 与 `DEFAULT_CRAFT_ZONE_ID`，制作成本统一执行 `player → pending_inflow_account()`。
- [x] P0：不在制作入口自行解析或直写 zone；后续由 `world::heartbeat::zone_qi_inflow_tick` 按 equilibrium、速率、负灵域与坍缩事件规则执行 `pending → zone`。
- [x] P1：新增缺少空间上下文的生产入口回归，锁定制作仍成功、待分配池等额增加、`zone:spawn` 恒为零、账本总量不变。

原骨架的“按玩家脚下真实 zone 立即结算”方案已被后续归档的 `plan-zone-qi-economy-v1` 明确取代：待分配池是全服共享来源，zone 回流必须由 heartbeat 统一调度。若在制作入口立即灌入当前位置，会绕过流速/平衡点/负灵域/坍缩门禁并突破 zone 容量上限。

## 7. 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 至少新增/更新以下测试：
  - `apply_craft_intents_uses_current_zone_for_crafting_qi`
  - `apply_craft_intents_uses_current_dimension_for_tsy_zone`
  - `apply_craft_intents_does_not_fallback_to_spawn_when_zone_missing`
- 联调 gate：若改动触及 C2S craft intent，可补 bot e2e craft 场景，验证黑盒玩家在非 spawn 区域启动正 `qi_cost` 配方后不会把账记到 spawn。

## 8. 对抗复核结论

第一轮反方质疑指出：候选若写成“吞真元/全局守恒破坏”或“`start_craft` 底层硬编码 spawn”会误报；`start_craft` 本身是参数化并做 `WorldQiAccount::transfer`，全局账本仍零和。

第二轮修正后，反方最终裁决为高置信成立：生产 C2S `CraftStart` / `CraftStartIntent` 不带 zone，服务端必须由 `Position + CurrentDimension + ZoneRegistry` 推导；当前 `apply_craft_intents` 固定传 `"spawn"`，导致非 spawn 区域正 `qi_cost` craft 稳定落到 `zone:spawn`。本 plan 严格收边界为“Crafting 真元目标 zone 错误”，不声称吞真元、不声称全局守恒破坏、不声称 `ZoneRegistry.spirit_qi` 已被立即改写。

实施期复核进一步发现，`plan-zone-qi-economy-v1` 已把所有玩法消耗的权威目标改为独立共享待分配池，再由 heartbeat 统一分配。最终修复因此选择删除陈旧 zone 参数，而不是在 craft 模块重建一条平行的 zone 结算路径。

## Finish Evidence

### 落地清单

- `server/src/craft/session.rs`：`StartCraftRequest` 删除 `zone_id`；`start_craft` 只向 `pending_inflow_account()` 转账。
- `server/src/network/craft_emit.rs`：删除硬编码 `DEFAULT_CRAFT_ZONE_ID = "spawn"`，补生产入口守恒回归。
- `server/src/qi_physics/ledger.rs`：`QiTransferReason::Crafting` 文档与待分配池/`ZoneInflow` 两阶段语义对齐。

### 关键 commit

- `739dcb7b`（2026-07-13）：移除制作起手的陈旧区域契约。
- `4181b1e1`（2026-07-13）：锁定制作入口只向待分配池入账。
- `edc44c93`（2026-07-14）：普通 merge 至最新 `origin/main`（含 dying-elder 与 race-system P4）。

### 测试结果

- `cargo test craft --lib`：287 passed，0 failed（恢复正确实现并合并 `origin/main` 后）。
- `cargo fmt --check`：通过。
- `git diff --check`：通过。
- 完整 `cargo clippy --all-targets -- -D warnings && cargo test`、e2e 与云端 review 结果在 PR 门禁补录。

### 跨仓库核验

纯 server 修复，无 schema/agent/client wire 变更。守恒链路为 `QiTransferReason::Crafting`（player → pending）→ `QiTransferReason::ZoneInflow`（heartbeat pending → zone）；制作模块不直写 `zone.spirit_qi`。

### 遗留 / 后续

无功能遗留；zone 的具体回流目标与速率继续由 `plan-zone-qi-economy-v1` 的 heartbeat 机制统一负责。
