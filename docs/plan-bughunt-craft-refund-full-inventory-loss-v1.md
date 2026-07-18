# plan-bughunt-craft-refund-full-inventory-loss-v1

> **active bughunt plan（归档审计发现 P4 验收缺口）**。一句话主题：修复 `craft` 显式取消或产物入包失败后的材料退款在背包已满时只记录日志、却不入包、不落地且删除 `CraftSession`，导致应返材料永久丢失的问题。

## 当前状态

| 阶段 | 主题 | 当前交付物 / 可核验锚点 | 状态 |
|------|------|-------------------------|------|
| P0 | 钉死退款事务语义 | `server/src/network/craft_emit.rs::grant_refund_manifest_to_inventory_or_ground`；clone staging 后整批发布，结构错误整批回滚 | ✅ 2026-07-12 |
| P1 | 接入满包地面兜底 | `DroppedLootRegistry`、`InventoryInstanceIdAllocator`、`refund_ground_context`、`add_item_to_player_inventory_or_ground`；玩家位置与 `CurrentDimension` 原样进入落地点 | ✅ 2026-07-12 |
| P2 | 按实际结果回报退款数 | `apply_craft_cancel_intents` 将 `CraftFailedEvent.material_returned` 改写为实际入包数 + 实际落地数；成功持久化后才发布事件并删除 session | ✅ 2026-07-12 |
| P3 | 闭环 finalize、持久化与重连 | `tick_craft_sessions`、`persist_dirty_craft_sessions`、`save_player_craft_checkpoint`、`hydrate_durable_inventory_state`、玩家 join/disconnect/shutdown 恢复与保存 | ✅ 2026-07-12 |
| P4 | 饱和回归与生产 Bot 验收 | 满包、mixed、缺 registry、unknown template、allocator、持久化、重连及 Bot 链路已有覆盖；仍缺原 plan 点名的 `no containers` helper + cancel/finalize 可达链 pin | ⏳ |

> **归档门未满足**：运行时代码中，`server/src/inventory/mod.rs::add_item_to_player_inventory_or_ground` 仅对 `inventory full:` fallback，`add_item_to_player_inventory_inner` 在 `carried_container_candidate_indices(...).is_empty()` 时返回 `player inventory has no containers`；因此 `no containers` 保持为结构错误。craft 退款 helper / cancel / finalize 的 clone staging 与 error 分支从代码上会回滚并保留 `CraftSession`。但原 plan 明确要求以定向测试锁死该分支；截至 PR #1142 final head 及当前 `origin/main`，没有 `no containers` 专属 helper、cancel、finalize 回归。因此 P4 不能标记完成，本 plan 不能迁入 `finished_plans/`。

## Bug 摘要

- **类型**：真实 gameplay bug，`fix_pr`。
- **原始范围**：`server/src/craft/session.rs`、`server/src/network/craft_emit.rs`、`server/src/inventory/mod.rs`。
- **一句话根因**：`cancel_craft()` 只计算退款清单；原发放路径使用裸 `add_item_to_player_inventory`，满包失败后仍发送预计算结果并移除 `CraftSession`，没有 `DroppedLootRegistry` fallback、pending retry 或原子 rollback。
- **非重复性**：这不是 `plan-craft-close-pause-loss-v1` 的“关闭 UI 被误当显式取消，产生设计内 30% 损耗”；本题是“已经进入退款语义后，设计应返还的 70% 又因背包已满而被吞掉”。

## 接入面与契约锚点

- **进料**：`CraftStartIntent` 预扣配方材料并建立 `CraftSession`；`CraftCancelIntent` 或 `tick_craft_sessions` 的产物 grant 失败生成 `refund_manifest`；`PlayerInventory`、`ItemRegistry`、`Position`、`CurrentDimension` 提供退款上下文。
- **出料**：退款优先写回 `PlayerInventory`；背包已满时写入 durable `DroppedLootRegistry`；`CraftFailedEvent.material_returned` 与 `CraftOutcomeV1::Failed.material_returned` 只统计真实入包或落地成功数；成功检查点提交后才终结 session。
- **共享类型 / event**：复用 `CraftSession`、`CraftFailedEvent`、`PlayerInventory`、`DroppedLootEntry`、`DroppedLootRegistry`、`InventoryInstanceIdAllocator`、`CraftSessionPersistenceDirty`，没有新增近义退款事件或第二套掉落 registry。
- **持久化契约**：SQLite migration v36 `player_craft_sessions`、v37 `dropped_loot`；`save_player_craft_checkpoint` 原子提交 inventory/session/可选 cultivation/qi ledger/durable drops；拾取走 `save_player_inventory_and_delete_dropped_loot` 原子提交 inventory 与 durable row 删除。
- **跨仓库 / wire 契约**：沿用 `proto/bong/envelope.proto` 的 `craft_session_state`（tag 22）、`dropped_loot_sync`（tag 81）及 `CraftOutcomeFailed.material_returned`（field 5）；server 由 `CraftSessionStateV1` / `CraftOutcomeV1` 发包，Bot 由 `scripts/bot/proto_min.py` 解码并在 production scenarios 断言。PR #1142 未修改 client、agent 或 proto/schema 定义。
- **worldview / qi_physics**：本题只修既有物品退款与持久化，不新增世界观机制或真元流动；worldview 锚点 N/A，qi_physics 锚点 N/A。
- **玩家可感知行为**：本题是纯 server 正确性修复，客户端协议和既有 UI/A/V 不变。成功入包继续由 inventory snapshot 表现，满包 fallback 继续由既有 `dropped_loot_sync` 表现，失败数量继续使用既有 craft outcome；不新增 HUD、粒子、音效、动画、环境或 narration。

## 原始可达性与影响

1. `start_craft()` 在创建 session 前预扣材料，但不锁格、不预留退款容量。
2. session active 期间，拾取、交易、库存整理或异步奖励可重新填满腾出的空间。
3. 玩家显式取消，或产物 grant 因满包失败而进入剩余批次退款。
4. 修复前裸入包返回 `inventory full: <template>`；调用方只记录日志，却仍删除 `CraftSession`。
5. 玩家可能看到预计算的“已返还 N 个”，但背包与地面均没有材料；稀缺材料、长配方和批量制作会在取消税之外二次受损。

两轮原始反方审查均未能推翻：server 没有 craft 期间的统一库存冻结；原路径没有 dropped-loot 资源；PR #1030 是 craft outcome 网络反馈，PR #1034 是炼丹取丹满包，均不覆盖本题。

## 开放问题（历史回填）

原 active plan 未在实施前按现行模板单列开放问题；归档审计从最终实现反推并记录以下四个历史决策门。§1.1-§4.1 的设计决策均已收口，但 P4 仍有一项明确验收缺口：`no containers` 结构错误尚无 helper + cancel/finalize 可达链 pin 测试。

- 背包已满时，退款应落地还是保留 pending refund？
- mixed manifest 遇到后项结构错误时，已成功前项是否允许部分提交？
- 缺 `DroppedLootRegistry`、recipe 或持久化失败时，何时允许终结 session？
- 重复 intent、断线重连、服务重启与掉落拾取如何维持 exactly-once？

## 已落地决议

### §1.1 决议：背包已满时退款去向

**结论 / 实施方案**：退款项必须“入包或落地”；只允许库存容量不足走既有 `add_item_to_player_inventory_or_ground`，不得只写日志后丢弃。

**边界条件**：unknown template、`player inventory has no containers`、instance ID 越界/碰撞等结构错误不能伪装成掉落成功。`no containers` 的实现分类已存在，但 P4 专属测试尚未补齐。

**落点**：`server/src/network/craft_emit.rs:156-226`（`refund_ground_context`、`grant_refund_manifest_to_inventory_or_ground`）/ 本 plan P0-P1。

### §2.1 决议：mixed manifest 与结构错误

**结论 / 实施方案**：inventory、allocator、dropped-loot registry 全部 clone staging；manifest 任一结构错误即整批不发布，并把实际返还数清零，防止重试复制前项或遗留部分掉落。

**边界条件**：满包且 registry 可用是正常 fallback；缺 registry、unknown template、`no containers`、allocator 边界和已有 drop ID 碰撞均是可诊断错误。现有测试覆盖除 `no containers` 外的列举分支；该缺口见 P4。

**落点**：`server/src/network/craft_emit.rs:170-226` 及 `refund_manifest_*` 回归 / 本 plan P0-P1、P4。

### §3.1 决议：缺少 registry、recipe 或持久化失败

**结论 / 实施方案**：不得终结退款凭证。缺 `DroppedLootRegistry`、缺 recipe 或 SQLite 检查点失败时保留 `CraftSession`/dirty 状态，不发 terminal outcome；依赖恢复后重试一次并只提交一次。

**边界条件**：只有 inventory/session/durable drops 的统一检查点提交成功后，才发布 staging、发送终态事件并删除 session。

**落点**：`server/src/network/craft_emit.rs:396-731`（`apply_craft_cancel_intents`、`tick_craft_sessions`）、`:732-761`（`persist_dirty_craft_sessions`）、`server/src/player/state.rs:766-817` / 本 plan P2-P3。

### §4.1 决议：重复请求、断线与重启

**结论 / 实施方案**：同帧重复 start/cancel 先去重；断线和停服保存 session，登录恢复 session；durable drop 启动 hydrate 并推进 allocator high-water，拾取时 inventory 写入与 durable row 删除同事务。

**边界条件**：重复 hydrate 不复制掉落，重连后已终结 session 不复活，拾取事务失败则 inventory/drop/zone 一起回滚。

**落点**：`server/src/network/craft_emit.rs:396-731`、`server/src/inventory/mod.rs:899-930`、`server/src/player/mod.rs:193-525`、`server/src/player/state.rs:819-855` / 本 plan P3-P4。

## 验收覆盖

- **满包 / mixed / 实际计数**：`refund_manifest_full_inventory_drops_to_ground`、`refund_manifest_mixed_grant_and_drop_counts_actual_returned`、`cancel_refund_full_inventory_drops_to_ground_and_reports_actual_returned`。
- **缺 registry / missing-unknown 配置**：`refund_manifest_full_inventory_without_registry_reports_error_without_counting_returned`、`refund_manifest_unknown_template_does_not_create_ground_drop`、`cancel_refund_missing_drop_registry_preserves_session_then_retries_once`、`cancel_intent_unknown_recipe_preserves_session_without_terminal_event`、`finalize_missing_recipe_preserves_completed_session_without_terminal_event`。
- **原子 rollback**：`refund_manifest_structural_error_rolls_back_earlier_grants_atomically`、`refund_manifest_allocator_boundary_rolls_back_without_drop_id_collision`、`refund_manifest_rejects_existing_drop_id_collision_without_overwrite`、`start_persistence_failure_keeps_inventory_qi_ledger_and_session_at_pre_state`、`craft_checkpoint_rolls_back_every_slice_when_durable_drop_write_fails`、`pickup_checkpoint_rolls_back_inventory_drop_and_zone_together`。
- **finalize**：`finalize_failure_refund_full_inventory_drops_to_ground_without_bone_coin_drift`。
- **persistence / reconnect / idempotency**：`duplicate_start_intents_same_frame_consume_materials_only_once`、`duplicate_cancel_intents_same_frame_refund_only_once`、`inventory_and_craft_session_roundtrip_and_clear_atomically`、`durable_craft_drop_roundtrips_seeds_allocator_and_stays_deleted_after_pickup`、`disconnect_flush_persists_latest_player_slices_before_cleanup`、`shutdown_flush_persists_connected_player_slices_without_disconnect`。
- **协议级生产链路**：`production_craft_cancel_full_inventory_refund.py` 验证同帧双 cancel 只退款一次、两份 durable drop 跨重连存活并可逐份拾取；`production_craft_disconnect_resume.py` 验证 session 断线暂停、同值恢复、取消后 exactly-once 退款且再次重连 inactive。

## P4 未完成验收门

原 active plan 的 `refund_structural_error_does_not_mask_config_bug` 明确点名 `unknown template / no containers`。当前实现已经具备正确分类和保留凭证的代码形状，但测试只锁住了 unknown template 等相邻分支，尚不能用等价推断替代以下专属回归：

- **helper pin**：构造 `PlayerInventory.containers.is_empty()`，调用 `grant_refund_manifest_to_inventory_or_ground`；断言错误包含 `player inventory has no containers`，`material_returned/granted_count/dropped_count == 0`，不创建 `DroppedLootEntry`，且 inventory revision、allocator、registry 均不发布 staged 变化。
- **cancel 可达链 pin**：无容器玩家显式取消；断言不发送 terminal `CraftFailedEvent`、不创建内存或 durable drop、实际返还为 0，`CraftSession`/退款凭证保留以供配置修复后重试。
- **finalize 可达链 pin**：无容器使产物 grant 失败并进入退款；断言不发送 `CraftCompletedEvent` 或 terminal failed outcome、不发布 staged inventory/allocator/registry，完成边界 session 保留且可持久化重试。

本归档审计的原始授权禁止修改产品代码/测试，因此这里只恢复 active 状态并如实记录门禁；后续实现 PR 补齐以上测试并通过 server 完整 gate、fresh exact-HEAD validator 后，才可重新归档。

## 当前核验证据

- **落地清单**：
  - `server/src/network/craft_emit.rs`：`grant_refund_manifest_to_inventory_or_ground`、`apply_craft_cancel_intents`、`tick_craft_sessions`、`persist_dirty_craft_sessions`，覆盖入包/落地、mixed manifest、结构错误 rollback、missing registry/recipe retry、实际 `material_returned`、同帧幂等与 finalize。
  - `server/src/persistence/mod.rs`：migration v36 `player_craft_sessions`、v37 `dropped_loot` 及 durable dropped-loot CRUD/high-water 查询。
  - `server/src/player/state.rs`：`save_player_craft_checkpoint` 与 `save_player_inventory_and_delete_dropped_loot` 两类 SQLite 原子事务及故障注入/重启回归。
  - `server/src/inventory/mod.rs`、`server/src/player/mod.rs`、`server/src/network/mod.rs`：durable hydrate、allocator high-water、join/disconnect/shutdown 生命周期和 craft 系统发布顺序。
  - `scripts/bot/proto_min.py`、`scripts/bot/test_protocol.py`、`scripts/bot/scenarios/production_craft_cancel_full_inventory_refund.py`、`scripts/bot/scenarios/production_craft_disconnect_resume.py`：wire 解码与生产黑盒闭环。
- **关键 commit / PR**：PR [#1142](https://github.com/Kizunad/Bong/pull/1142) 于 2026-07-13 合并；其产品修复 final head 为 `89be0411a752c1a4e559e2fe072ab8eb74a6f8d5`，merge commit 为 `1b5fad889273a07be0bc459a470edbdc676cf3d2`。核心提交：`eb23c120ae05c211a58829de7ba034e90317e2e4`（满包落地兜底）、`26d2f411ded9a71143a338de36ff589b22d217f0`（取消路径回归）、`145ddeb0aa0eaeb47ebbc379a8b45f5aa329b5de`（重复取消幂等）、`36f0923c9cd2edbcb9b3c06d010f004a8189d4e9`（持久化检查点）、`8ae05978f0aa469d2cb40f6739d8ae15d7c5c276`（运行时守恒）、`364a678424ca54b932e8da7b24451a1221e9a779`（生产链路证据）、`07042372f08cef37a64d78eff4706a712d730e8e`（拾取原子持久化）、`89be0411a752c1a4e559e2fe072ab8eb74a6f8d5`（Bot 用户名修正）。
- **PR #1142 测试结果**：[E2E run 29214120063](https://github.com/Kizunad/Bong/actions/runs/29214120063) 的 `head_sha` 明确为产品修复 final head `89be0411a752c1a4e559e2fe072ab8eb74a6f8d5`；该 run 成功，artifact `e2e-evidence` ID `8266243303` 也属于同一 run。原始 workflow log 显示 server `cargo test`：lib 11400 passed / 0 failed / 1 ignored，main 11/11，full-app 1/1，Tarkov e2e 4/4，doc tests 0 failed / 5 ignored；`craft_emit.rs` 当时 39 个 `#[test]`、`craft/session.rs` 41 个 `#[test]`。Bot protocol 51/51；`smoke-test-e2e.sh` 8/8；Bot e2e 26/26，其中目标场景分别 2.9s 与 3.2s PASS。该 final-head workflow 另含 proto lint、Java 17 client test、schema build/check/test、agent check/test、release server build，均成功。
- **本次归档审计**：归档 PR #1232 初始 exact HEAD `384a871afa19d2b0bc955e1bb25c2ab74034942a` 曾由全新、无上下文、read-only validator 在第一步对拍后给出 PASS；随后紧邻执行 `git fetch origin && git merge origin/main`，结果为 Already up to date，HEAD 未变化。后续 `/review` 重新对照原 active plan，发现 P4 点名的 `no containers` 专属验收未落地，推翻了“可归档”的结论；因此本轮恢复 active，初始 PASS 只作为审计历史，不再作为 finished 门禁。run `29214120063` / artifact `8266243303` 仍只绑定上一条的 PR #1142 产品修复 SHA。
- **历史 review / CI 事实**：CodeRabbit 三轮分别发布 3、2、7 条 actionable comments；前两轮代码/测试项已在后续提交处理，最终 7 条均为 plan 可核验性与生命周期问题（接入面、实现 symbol、措辞、纯 server A/V 声明、决议、exact SHA/CI、归档），由本 active 文档保留审计事实。Review Action runs [29191618111](https://github.com/Kizunad/Bong/actions/runs/29191618111)、[29214122629](https://github.com/Kizunad/Bong/actions/runs/29214122629) 因 reviewer HTTP 400 safety filter 降级，[29214126756](https://github.com/Kizunad/Bong/actions/runs/29214126756) 因 circuit preflight skipped 降级；workflow 评论均明确标为 infrastructure failure、不是代码 finding。
- **跨仓库核验**：server `CraftSessionPersistenceDirty` / `save_player_craft_checkpoint` / `DroppedLootRegistry` / `CraftOutcomeV1::Failed.material_returned`；wire `craft_session_state` / `dropped_loot_sync` / `material_returned`；Bot 对三者解码并走真实 server 场景。PR #1142 未修改 client、agent 或 proto/schema 定义。
- **重复 skeleton 处置**：`docs/plans-skeleton/plan-bughunt-craft-refund-full-inventory-loss-v1.md` 与被 PR #1142 消费的原 active 文档拥有同一 bug 摘要、证据、触发路径、反方裁决、P0-P4、验收和风险；skeleton 仅保留“未实施/只立骨架”的旧状态，没有 active 主文档未覆盖的额外交付物。因此保留本 active 主文档，仅删除同 basename 的滞后重复 skeleton。
- **遗留 / 后续**：本 plan 唯一阻塞项是 P4 的 `no containers` helper + cancel/finalize 可达链 pin；补齐并通过门禁后才可归档。相邻的 UI close/cancel 语义属于 `plan-craft-close-pause-loss-v1`；未来若改变 craft outcome 或 dropped-loot wire，须继续维持“实际返还计数 + durable exactly-once”契约。
