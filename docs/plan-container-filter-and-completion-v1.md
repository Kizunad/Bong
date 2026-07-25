# plan-container-filter-and-completion-v1 — owner-instance 容器筛选与九个随身容器闭环

> **主题**：在当前 `ContainerState.owner_instance_id` + `PlayerInventory.containers` 平展模型上，统一容器 owner/filter 解析、权威移动门、保鲜状态迁移、九个随身容器数据与客户端预提示。
> **状态**：Active。P0 已于 2026-06-13 合入；P1 有零散代码超前，但 `InventoryMoveIntent` 权威玩家移动链尚未闭环，因此本 plan 仍在实施中。
> **历史证据**：P0 = PR #526，merge commit `3161ccf0ba1ff25d5ab781e654667090b0e143ac`（2026-06-13）。该证据只覆盖 P0 明列的数据模型与测试，不外推 P1–P4 完成度。

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | `ItemCategory` 三变体 + `ContainerAcceptFilter` + `ContainerSpec.accept_filter` + TOML `accept` + `item_passes_filter` | ✅ | 2026-06-13 |
| P1 | `PreparedInventoryMutation` 预提交事务、`classify_container_ownership` / `find_live_container_owner`、所有端点门与 live rebuild/spill | 🔄 | |
| P2 | `apply_container_freshness_transition` prepare fn、持久化 `integrity_lock`、`reconcile_container_integrity_freshness` | ⬜ | |
| P3 | 暗器 category 迁移 + 九件完整 `ContainerSpec`、coin 文案与 registry/save-load 闭环 | ⬜ | |
| P4 | `ContainerSnapshotV1.accept_filter` / persisted `acceptance_lock` 跨端同步 + Worn/Pack/Inspect 三 surface、e2e 与同 PR 归档 | ⬜ | |

> P1 的 `🔄` 仅表示 `container_accepts_runtime_grant` 已有部分 owner/filter 代码：权威玩家 move 仍未使用同一 resolver，swap 也未双向执行 filter 校验。不得据此将 P1 标成完成。

## 接入面（防孤岛）

- **进料**：
  - `server/src/inventory/mod.rs::PlayerInventory.containers`：所有运行时容器的平展集合。
  - `server/src/inventory/mod.rs::ContainerState.owner_instance_id`：随身容器到 owner 物品实例的唯一权威归属。
  - `server/src/inventory/mod.rs::{ContainerSpec.accept_filter, ContainerAcceptFilter, item_passes_filter}`：P0 已落地的筛选定义与纯判定函数。
  - `find_live_container_owner` 唯一从 worn、held、hotbar、`body_pocket` 寻找 live owner；`main_pack` / `small_pouch` / `front_satchel`、任意 `pack_*` grid 与 owner 自身容器都不属 eligible surface。
  - `server/src/schema/client_request.rs::ClientRequestV1::InventoryMoveIntent` → `server/src/network/client_request_handler.rs::handle_inventory_move` → `server/src/inventory/mod.rs::apply_inventory_move_with_race`：玩家移动的唯一权威链。
  - `server/src/shelflife/container.rs::{container_storage_multiplier, enter_container, exit_container}` 与 `server/src/spiritwood/mod.rs::item_freshness_behavior`：既有保鲜公式和 Freeze 时间记账入口。
- **出料**：
  - runtime grant、move 与 swap 共用 `classify_container_ownership` / `find_live_container_owner` / owner-filter 解析；所有 `InventoryLocationV1::Container` 源、目标端点均先过结构门，源端不对移出物作 accept filter。
  - P1 只经 `prepare_inventory_mutation_with` 生成尚未触碰 live inventory 的 `PreparedInventoryMutation`，并且仅由不可失败的 `commit_prepared_inventory_mutation` 一次性写入 detach/attach/merge/push/equip/hotbar、staged 字段与 revision；正常 owner 位置变化在同一 P1 commit 原子 rebuild/spill/remove。
  - P1 生产调用传 identity `InventoryItemPrepareFn`；P2 将同一 prepare fn 替换为强制名 `apply_container_freshness_transition`，在 merge 选择前作用于每个 incoming logical item 的 staged copy，closure 捕获权威 `now_tick` 与 from/to behavior。
  - `ContainerState.integrity_lock: Option<ContainerIntegrityLock { reason, detected_tick: u64 }>` 由 `reconcile_container_integrity_freshness` 持久化；P2 只让既有 server snapshot emit 在 reconciliation 后读取 server 状态，P4 才将 persisted lock 映射并下发 `acceptance_lock`，供 `WornContainerPanel`、`PackContainerWindow`、`InspectScreen` 做非权威预提示。
- **共享类型 / event**：复用 `ItemCategory`、`ContainerAcceptFilter`、`ContainerFreshnessBehavior`、`InventoryMoveRejectReason`、`InventoryMoveOutcome`、`InventoryLocationV1`；不另造第二套移动协议或容器状态树。
- **跨仓库契约**：
  - server：`ContainerSnapshotV1` / `InventoryItemViewV1`、`inventory_snapshot_emit::build_inventory_snapshot`、`schema::proto_convert`。
  - schema：`agent/packages/schema/src/inventory.ts::{ContainerSnapshotV1, InventoryItemViewV1}` + inventory snapshot sample；agent 无玩法逻辑，但 TypeBox 仍是 wire 镜像的一部分。
  - protobuf：`proto/bong/envelope.proto::{ContainerSnapshot, InventoryItemView}`。
  - client：`InventorySnapshotHandler` → `InventoryModel.ContainerDef` / `InventoryItem` → `InventoryStateStore` → `WornContainerPanel` / `PackContainerWindow` / `InspectScreen`。
- **worldview 锚点**：保鲜容器呼应 `worldview.md §二 L157-L163` 的离体真元易挥发；`coin_box` 只做骨币品类筛选，遵守 `worldview.md §九 L844-L850` 的骨币流转约束，不增加骨币保鲜或囤积收益。
- **qi_physics 锚点**：本 plan 只选择既有 `ContainerFreshnessBehavior` 并维护既有冻结时间字段，不定义任何衰减率公式或新的物理常数。过滤与移动只改变库存归属，不生成、吞噬或重复结算真元/灵气；若实现发现必须改变真元量，必须停下并接 `qi_physics::ledger::QiTransfer` 既有守恒入口，而非在本 plan 内私设扣减或返还公式。

## 当前架构基线与范围边界

1. `ContainerState.owner_instance_id` 是 owner 真源；`pack_<instance_id>` 仅保留为稳定 UI / 协议 `container_id`。实现只能用它与 owner 字段核对，绝不得从 ID 后缀反推出或补写权威 owner。
2. `classify_container_ownership` 是唯一分类入口，静态 allowlist 固定且穷尽为 `body_pocket`、`main_pack`、`small_pouch`、`front_satchel`（锚：`server/src/inventory/mod.rs:83-107,6549-6592`）。它不接受“已知”“预期”等推断分类：
   - allowlist + `owner_instance_id=None` → `Static`，`accept_filter=None`，全收（`Normal` freshness）；
   - allowlist + `owner_instance_id=Some(_)` → `ContainerOwnerInvalid`；
   - 非 allowlist + `owner_instance_id=None`（未知 ID、伪 `pack_` 文本、placeholder 都包括）→ `ContainerOwnerMissing`；
   - 非 allowlist + `owner_instance_id=Some(owner_id)` → 只按该字段调用 `find_live_container_owner` 查实例，再查 template/`ContainerSpec`，并核对 ID 必须为 `pack_<owner_id>`；任一缺失、无效或不匹配均 typed reject。
3. `find_live_container_owner` 是唯一 owner lookup surface，供 classify/resolve、P1 rebuild、grant、move、swap、P2 freshness/reconcile/sweep、snapshot 共同复用：只允许 worn、held、hotbar、`body_pocket` 命中 owner instance；明确排除 `main_pack` / `small_pouch` / `front_satchel`、任意 `pack_*` grid 与 owner 自己的容器。eligible surface 无匹配 instance → `ContainerOwnerNotFound`；匹配实例但 registry template 缺失或无 `ContainerSpec` → `ContainerOwnerInvalid`。不得从 ID 推 owner。
4. 所有 `InventoryLocationV1::Container` 端点（from/to）必须在其它语义校验和 mutation 前走同一 classify/resolve：源端只校验 ID/owner 结构，绝不对移出的物品应用 accept filter；目标端再校验结构和 accept filter。普通 move、swap、container→hotbar 与 container→equip 全部适用。
5. `PreparedInventoryMutation` 是唯一库存事务载体：只持有受影响 item/container/equip/hotbar 的 staged copies、from/to route、候选 stack merge/push 计划与原 revision；在 `commit_prepared_inventory_mutation` 前严禁触碰 live inventory。正常 owner 位置变化由 P1 的同一准备/提交事务调用 rebuild：owner 离开 eligible surface 时原子 spill/remove 对应容器，不得留下 locked container。
6. `water_skin` 已移出本 plan scope：不为它定义 grid、filter、`ContainerSpec`、随身包验收或任何剩余 PR 交付物。
7. `trade_crate` 与 `dead_drop_box` 属 placeable container 计划，不计入本 plan 的九个随身容器，也不在本 plan 实装方块放置、打开或持久化。`herb_crate_placed` 是 `herb_crate` 的独立 placeable twin：P3 不修改它，也不实现 portable↔placed 转换。

## 历史否决：不得恢复 nested/session 路线

旧设计曾依赖 `ItemInstance.sub_container`、`MAX_PACK_NEST_DEPTH`、`PackItemSession`、`PackContainerOpen`、`PackContainerMove`、`PackContainerClose` 与 `SubContainerPanel`。这些符号在本节仅作否决记录：P1–P4 不得恢复它们，不得新增旧 session wire 的兼容分支，也不得把它们写成依赖、TODO、测试目标或验收入口。当前唯一移动入口始终是 `InventoryMoveIntent`。

## P0 — 筛选数据模型 ✅ 2026-06-13

**已完成范围仅限以下交付物**（PR #526 / `3161ccf0ba1ff25d5ab781e654667090b0e143ac`）：

- `server/src/inventory/mod.rs::ItemCategory::{Mineral, Anqi, Liquid}` 及 TOML category 解析。
- `server/src/inventory/mod.rs::ContainerAcceptFilter::{Category, TemplatePrefix}`。
- `server/src/inventory/mod.rs::ContainerSpec.accept_filter` 与 TOML `[item.container].accept` 解析。
- `server/src/inventory/mod.rs::item_passes_filter`：`None` / empty 全收；非空列表按 OR 语义匹配 category 或 template prefix。
- 对应 category、TOML、serde、默认值、单 filter、多 filter 与正反路径测试。

**不属于 P0 完成证据**：runtime grant 接线、玩家移动校验、owner 损坏拒绝、freshness、九容器数据、snapshot/client wire、UI 提示与 e2e。上述全部留在 P1–P4。

## P1 — owner/filter 统一 resolver 与权威移动门 🔄

### 可核验交付物

1. 在 `server/src/inventory/mod.rs` 冻结唯一 `classify_container_ownership` 和其上的 `resolve_container_acceptance`：输入 `&PlayerInventory`、`&ItemRegistry`、`container_id`，分类严格遵循「当前架构基线」的四行真值表；输出目标 `ContainerState`、`Resolved` 或 `Locked` 及归一化 filter。它只能经 `find_live_container_owner` 查询 owner，静态 allowlist 只能是 `body_pocket` / `main_pack` / `small_pouch` / `front_satchel`，所有 owner-backed 判断只读 `ContainerState.owner_instance_id`。P1 固定内部 `ContainerAcceptanceLockReason::{OwnerMissing, OwnerNotFound, OwnerInvalid}`；同 PR 将 `rebuild_containers_from_equipment` 的 live 判据替换为该 helper，owner 从 eligible surface 离开时，在同一 mutation commit 原子 spill/remove，full overflow 走既有 drop outcome，不能先留下 locked container。
2. resolver 的接受语义严格为：
   - `Static` 的 `None` filter 全收，静态容器带 owner 一律 `InventoryMoveRejectReason::ContainerOwnerInvalid`；
   - 非 allowlist 无 owner 一律 `ContainerOwnerMissing`，包括未知 ID、伪 `pack_` 文本和 placeholder；
   - 非 allowlist 有 owner 时，eligible surface 没有 owner instance 为 `ContainerOwnerNotFound`；template 缺失、无 `ContainerSpec` 或 `container_id != pack_<owner_id>` 为 `ContainerOwnerInvalid`；
   - 仅完整 `Resolved` 的**目标端**非空 filter 未命中才是 `ContainerFilterRejected`，错误携带 `container_id`、目标物 `template_id` 与 filter 摘要。
3. 将现有 `container_accepts_runtime_grant` 改为 resolver 的薄包装：只把普通目标 filter mismatch 视为“跳过该候选容器”，结构损坏必须保留 typed error，不能把 owner 缺失、静态带 owner 或 ID-owner mismatch 当全收。
4. `PreparedInventoryMutation` 必须只在内存中持有受影响 item/container/equip/hotbar 的 staged copies、from/to route、候选 stack merge/push 计划与原 revision；未 commit 前不得触碰 live inventory。`prepare_inventory_mutation_with` 是唯一准备入口：先对所有 `InventoryLocationV1::Container` from/to 做结构 resolve（源仅结构，目标结构+filter），再对**每个 incoming logical item 的 staged copy**调用不可失败的 `InventoryItemPrepareFn`，然后按 prepare 后的完整 stack identity 重新计算 merge/push/placement，最后完成 bounds/collision/weight 等校验并返回 `PreparedInventoryMutation`；任何 `Err` 保证 live 状态零变。
5. `commit_prepared_inventory_mutation` 是唯一写边且不可失败：一次性应用 detach/attach/merge/push/equip/hotbar 与 staged item 字段，最后 revision 恰好 +1。P1 生产调用向 `prepare_inventory_mutation_with` 传 identity `InventoryItemPrepareFn`；禁止在 prepare/hook 前决定 merge，禁止任何后写补丁。
6. 普通 move、swap、container→hotbar/equip 与 runtime grant 全部走同一 prepare/commit 路线。任一 owner、filter、bounds、collision 或 footprint 校验失败时：`inventory.revision` 不变、每个 `ContainerState.items` 顺序与内容不变、装备与 hotbar 不变；grant 的最终失败同样零 layout 与 revision 副作用。
7. `None` 与 empty filter 始终全收；多个 filter 始终 OR，不引入 AND 或优先级语义。

### 饱和测试

- classification / resolver：四个静态 ID 分别 ownerless 全收；静态带 owner、未知 ownerless、伪 `pack_` ownerless、placeholder ownerless 各自 typed reject；合法 owner、eligible surface 缺失、template 缺失、无 `ContainerSpec`、ID-owner mismatch 各一条专属 case。
- unique live lookup / rebuild：worn、held、hotbar、`body_pocket` 四个合法 surface 各自命中；owner 从这四者分别移入 `main_pack`、任意 `pack_*` grid、自包含容器时，**同一** prepare/commit 原子 spill/remove，revision 恰好 +1；full overflow 走既有 drop outcome，绝不先遗留 locked container。
- filter：None、empty、目标 category 命中/拒绝、目标 prefix 命中/拒绝、多 filter 首项命中/末项命中/全不命中；源端移出任意 item 时不运行 accept filter。
- endpoints：locked 源→健康 container、locked 源→hotbar、locked 源→equip 各返回同类 typed owner reject；一般 filter/bounds 拒绝和每个上述拒绝均深比较 inventory/freshness 不变、revision 不变。
- `PreparedInventoryMutation` identity prepare：new stack、partial merge、full merge 三形态均在 identity `InventoryItemPrepareFn` 下覆盖；断言 hook 前不决定 merge、任何失败的 prepare 不触碰 live item/container/equip/hotbar/revision，成功 `commit_prepared_inventory_mutation` 一次性写布局并恰好 +1 revision；P1 不断言 Freeze 或其他 freshness 转换。
- 网络集成：真实 `ClientRequestV1::InventoryMoveIntent` → handler → prepare → commit → typed `InventoryMoveRejectedV1` / corrective snapshot，覆盖 happy、reject、swap、locked source 三路。

## P2 — owner-based freshness 与预提交状态迁移 ⬜

### 固定行为映射

| owner template | `ContainerFreshnessBehavior` | 语义 |
|----------------|------------------------------|------|
| `sealed_vial` | `Halve` | 非 Stepwise 的既有衰减速率减半 |
| `spirit_seal_box` | `Freeze` | 复用冻结时间记账；Stepwise 仍遵守既有 multiplier 语义 |
| `moisture_guard` | `SpoilOnly { rate: 0.3 }` | 只影响 Spoil track；Decay / Age 退 Normal |

### 可核验交付物

1. 扩 `server/src/spiritwood/mod.rs::item_freshness_behavior` 的 owner-template 映射，保留既有 `ling_xia` 与 `food.container.ice_cellar` 行为不变；不复制 `container_storage_multiplier` 公式。
2. 在 `server/src/inventory/mod.rs` 固定 `resolve_container_freshness_behavior`，复用 P1 `classify_container_ownership` / `find_live_container_owner`；静态容器默认 `Normal`，损坏 owner 返回同一 typed error。
3. P2 将 `prepare_inventory_mutation_with` 的生产 `InventoryItemPrepareFn` 强制替换为 `apply_container_freshness_transition`：closure 捕获权威 `now_tick` 与 from/to behavior，在 merge 选择前对每个 incoming logical item 的 staged copy 改写 freshness。move、swap、runtime grant 的所有成功形态随后均只由同一 `commit_prepared_inventory_mutation` 写入 staged 字段、merge/push/equip/hotbar 并 revision 恰好 +1；不得在提交后补写。
4. runtime grant 的 staged 顺序固定：先以目标 behavior 调用 `apply_container_freshness_transition`，再按完整 post-transition Freshness identity 决定 merge/push/placement，最后才做 bounds/collision/weight 校验与 commit。Freeze 仅可与 post-transition identity 完全相同的 stack 合并；非 freshness item 安全 no-op。此规则覆盖新 stack placement、部分 merge + new stack、全量 merge 既有 stack（`new_stacks` 为空但 stack_count 改变并 revision +1），不可绑定具体 `items.push`。
5. P2 在 `ContainerState` 与 inventory persistence 新增 `integrity_lock: Option<ContainerIntegrityLock>`，`ContainerIntegrityLock` 字段**恰为** `reason: ContainerAcceptanceLockReason` 与 `detected_tick: u64`，legacy/default=None；P1 固定内部 `ContainerAcceptanceLockReason::{OwnerMissing, OwnerNotFound, OwnerInvalid}`。冻结唯一函数 `reconcile_container_integrity_freshness`，统一从权威 `CultivationClock` 取得 `now_tick`，调用顺序为：load hydration 后 → 任一 grant/move/swap 前 → shelflife sweep 前 → snapshot emit 前 → 结构 typed reject 后。
6. `reconcile_container_integrity_freshness` 的状态机固定：
   - 首次 Healthy/None→Locked(reason)：写入 `{ reason, detected_tick: now_tick }`；容器内每个 `frozen_since_tick=Some(_)` 直接清为 None，**不**增加 `frozen_accumulated`，保守丢弃整个 active interval；items 的位置/stack/layout 不变，整批 reconcile revision 恰好 +1。
   - Locked 同 reason 重复检测：零修改、revision 不变；reason 改变：更新 reason/detected_tick，若仍有 active since 直接 discard，整批 revision 恰好 +1。
   - Locked→Resolved repair：同一原子 repair commit 清 `integrity_lock`；若修复后 behavior=Freeze，对每个 Freshness item `enter_container(now_tick)` 重开 active interval，否则保持 None；owner 修复/rebuild/filter+freshness 合为该一次 commit，revision 恰好 +1，不能二次 bump。
7. snapshot/sweep 只消费已 reconcile 的 server 状态：lock 存在时 behavior 强制 `Normal`，既有 server snapshot emit 仍可无条件发出真实 layout/items/revision。首次结构 reject 若触发 reconciliation，业务 move 本身零写而独立 integrity transition revision 恰好 +1，既有 corrective snapshot 带新 revision；同 reason 重复 reject revision 不变；一般 filter/bounds reject 仍 revision 不变。

### 饱和测试

- 三个固定映射逐一 pin，未知 owner 为 typed reject、静态容器为 `Normal`；既有 `ling_xia` / ice cellar 回归不变。
- `Halve` 覆盖 time-based 与 Stepwise；`Freeze` 覆盖两类公式；`SpoilOnly { rate: 0.3 }` 覆盖 Spoil / Decay / Age 三 track。
- `apply_container_freshness_transition` 作为 `prepare_inventory_mutation_with` 的强制 prepare fn，覆盖 move/swap/grant 的所有成功形态；hook 在 merge 前更改 staged copy，成功 `commit_prepared_inventory_mutation` 统一写 staged freshness/layout 并 revision 恰好 +1，绝不在提交后补写。
- runtime grant 覆盖 new stack placement、partial merge + new stack、full merge existing stack（`new_stacks` 为空但 stack_count 变化）三种成功形态；Freeze/非 Freeze、非 freshness no-op、post-transition Freshness identity 不同不得 merge、filter/full skip、owner corruption `Err` 与最终失败零副作用各自 pin。
- integrity persistence：legacy load 默认 lock=None；合法 Freeze 在 tick100、tick200 首次 lock、tick250 同 reason 重复、tick300 repair 的序列必须断言 active 100 起 interval 整段 discard（accumulated 不增）、重复 no-op、repair 从300重新 enter。三 reason、reason change、无 Freshness、batch 多 item、snapshot/sweep 对拍均专属 pin。
- revision 语义：首次 reconcile 与 reason change 各 +1；同 reason 重复 =0；repair（含 owner 修复/rebuild/filter/freshness）恰好 +1；首次结构 reject 的业务 move 零写但独立 reconcile +1 且 corrective snapshot 带新 revision；重复 typed reject、一般 filter/bounds reject 均 revision 不变。
- sweep/snapshot 只在 reconciliation 后运行；persisted lock 强制 Normal，任何 active Freeze interval 已被 discard，corrective snapshot 保留真实 layout/items/revision。

## P3 — 暗器迁移与恰好九个随身容器 ⬜

### 九容器完整 `ContainerSpec` 固定矩阵

> 以下字段是 P3 唯一 TOML 值，全部采用现有 `worn_grass_pouch` / `grass_pouch` 的 canonical profile（`8.0` / `0.008` 或 `10.0` / `0.005`），不留“实现时选择”空间；`equip_slot` 均为 parser 当前合法身体槽，`attrition_exempt=false`、`quick_access=false` 均须显式写入。

| # | template id | rows×cols | weight_capacity | equip_slot | durability_cost_per_op | attrition_exempt | quick_access | `accept`（OR） | freshness |
|---|-------------|-----------|-----------------|------------|------------------------|------------------|--------------|----------------|-----------|
| 1 | `herb_pouch` | 3×3 | 8.0 | `chest` | 0.008 | false | false | `herb`, `food` | Normal |
| 2 | `ore_sack` | 3×3 | 10.0 | `chest` | 0.005 | false | false | `mineral` | Normal |
| 3 | `projectile_bag` | 3×4 | 10.0 | `legs` | 0.005 | false | false | `anqi` | Normal |
| 4 | `herb_crate` | 4×4 | 10.0 | `chest` | 0.005 | false | false | `herb` | Normal |
| 5 | `sealed_vial` | 2×2 | 8.0 | `chest` | 0.008 | false | false | `pill`, `food` | Halve |
| 6 | `spirit_seal_box` | 2×2 | 10.0 | `chest` | 0.005 | false | false | `treasure`, `pill` | Freeze |
| 7 | `moisture_guard` | 3×3 | 8.0 | `legs` | 0.008 | false | false | empty（全收） | `SpoilOnly { rate: 0.3 }` |
| 8 | `coin_box` | 3×3 | 8.0 | `chest` | 0.008 | false | false | `bonecoin` | Normal |
| 9 | `sealed_envelope` | 1×2 | 8.0 | `head` | 0.008 | false | false | `recipe_fragment`, `recipe_hint`, `scroll` | Normal |

### 可核验交付物

1. 在现有 item TOML 中将上述恰好九个 template 迁为 `category = "container"`，并逐项按 P3 完整矩阵写齐 `[item.container]` 的 rows、cols、weight_capacity、equip_slot、durability_cost_per_op、attrition_exempt、quick_access、accept；不得省略默认字段或改为待调参数。它们接入 PR-1 已落地的 owner-instance / live rebuild 语义，不新增第二种容器状态。
2. `server/assets/items/anqi.toml` 中十二个暗器实物从 `misc` 迁到 `anqi`：`anqi_bone_chip`、`anqi_yibian_shougu`、`anqi_lingmu_arrow`、`anqi_dyed_bone`、`anqi_fenglinghe_bone`、`anqi_shanggu_bone` 及各自 `_charged` 变体。既有 `max_stack_count` 保持权威。
3. `projectile_bag` 的 category filter 必须对十二个迁移项全收、对非暗器拒绝；不得用 template-prefix 偷代 category 迁移。
4. `coin_box` 将现有“丹砂隔灵，骨币少漏”文案迁为 `骨币分类收纳匣，便于归类携带。`，只表达骨币分类/收纳便利；不得暗示隔灵、少漏、保鲜、保值或延缓半衰。`herb_crate_placed` 保持独立 placeable twin，P3 不修改它，也不实现 portable↔placed 转换。

### 饱和测试

- 九个 template registry-load 全绿；逐个断言 category、完整 `ContainerSpec` 八字段、filter、freshness 映射与矩阵一致。
- 每个 non-empty 容器各有一条 filter-invalid item 拒绝；`moisture_guard` 因 empty 全收不得有 filter-invalid item，负例只覆盖 bounds、collision、full 或 owner corruption，并另 pin empty 全收。
- 十二个暗器逐项断言 `ItemCategory::Anqi`，并逐项通过 `projectile_bag`；普通矿石、草药、骨币分别拒绝。
- save/load：九类容器按 PR-1 已落地的 eligible surface 语义恢复 owner、容量、内容与 filter 解析；普通/嵌套 grid 不产生 live container 的行为由 P1 回归锁住，P3 不重复实现 rebuild/spill。
- `coin_box` 文案测试 pin 仅骨币分类/收纳便利，明确不含“隔灵”“少漏”“保鲜”“保值”“半衰”等承诺。

## P4 — snapshot filter、客户端预提示与端到端验收 ⬜

### Wire 契约

1. 新增 `ContainerAcceptFilterV1`，JSON 采用稳定 tagged 形状：category = `{ "kind": "category", "value": "mineral" }`，prefix = `{ "kind": "template_prefix", "value": "anqi_" }`；protobuf 用等价 `oneof`。
2. 新增 `ContainerAcceptanceLockV1`，reason 仅可为 `owner_missing`、`owner_not_found`、`owner_invalid`；protobuf 用等价 enum 且 `NONE=0`。`ContainerSnapshotV1` 同时增加派生字段 `accept_filter: Vec<ContainerAcceptFilterV1>` 与 `acceptance_lock: Option<ContainerAcceptanceLockV1>`：lock=None 时 empty filter 才表示全收，不能用 empty 表示 corrupt owner。wire lock 必须由持久化 `ContainerState.integrity_lock.reason` 映射，不能临时从 snapshot projection 猜测。
3. `inventory_snapshot_emit::build_inventory_snapshot` 在 `reconcile_container_integrity_freshness(CultivationClock.now_tick)` 后直接下发 `ContainerState.owner_instance_id`、真实 revision/layout/items，并从同一 `find_live_container_owner` + resolver 构建 filter；`acceptance_lock` 直接映射 `integrity_lock.reason`。`pack_<instance_id>` 只作为 container ID，不再作为 owner 推导源。三种 owner 损坏路由必须形成可序列化 corrective snapshot；lock 存在时 snapshot/sweep 只消费已持久化的强制 `Normal` behavior，corrective snapshot 无条件可发。
4. 为客户端 category 判定补齐 `InventoryItemViewV1.category` 派生字段；它与 `template_id` 分别支持 category / prefix 两类 filter，禁止 client 维护硬编码 template→category 表。
5. 同步修改：
   - Rust serde structs、`server/src/schema/proto_convert.rs` 与正反测试；
   - `proto/bong/envelope.proto::{ContainerSnapshot, InventoryItemView}`；
   - `agent/packages/schema/src/inventory.ts`、samples，并先构建 `@bong/schema` dist 再跑 schema/agent 测试；
   - client `InventorySnapshotHandler`、`InventoryModel.ContainerDef`、`InventoryItem`、`InventoryStateStore`。

### Client 交互

1. `WornContainerPanel`、`PackContainerWindow` 与 `InspectScreen` 三个 surface 都从 `InventoryStateStore` 当前 `ContainerDef.acceptFilter` / `acceptanceLock` 读取；拖拽高亮统一调用固定 client 纯函数 `ContainerFilterRules.accepts`。作为源端时三 surface 不以 filter 阻断移出，但 locked source 仍显示锁态并在松手时发出原有 Intent。
2. `acceptanceLock != NONE` 时任意 drag **进入**该目标一律使用既有 `GridSlotComponent.HighlightState.INVALID`（`0x33CC2222`，即时显示、无 fade），filter 不再参与目标接受语义；无论 locked source 或 locked target，松手仍发送原有 `InventoryMoveIntent`，server 在源/目标结构门返回同类 typed owner reject。lock=None 时，dragged item 不通过目标 filter 同样显示 `INVALID`；通过 filter 后仍需叠加 bounds / collision 判定，只有全部通过才显示 `VALID`。
3. 预提示绝不代替 server 权威：即使 client 预测为非法，松手仍发送原有 `InventoryMoveIntent`；最终接受 / 拒绝、revision 与 corrective snapshot 全以 server 为准。corrective snapshot 不得丢弃或改用 empty filter 掩盖 corrupt owner。
4. 本阶段不新增粒子、动画、音频或 narration 资产；可感知反馈仅复用现有 grid `INVALID` / `VALID` tint，既有背包操作 A/V 不变。

### 饱和测试与验收

- Rust / TypeBox / protobuf：两种 filter 变体正反、unknown kind、缺 value、empty、OR 列表、owner 派生、静态全收、category 字段、serde/proto round-trip 与 sample 对拍；三种损坏 owner 路由的 corrective snapshot 亦必须可序列化，并携带真实 layout/revision/items 与来自 persisted lock 的对应 reason；snapshot/sweep 只读 reconciliation 后强制 `Normal` 状态。
- client parser：字段完整、empty、两变体、unknown kind fail-closed、非法 category；`acceptance_lock` 的三种 reason、snapshot 替换后 UI 读取最新 filter/lock，不持有旧 revision 数据；修复后的 snapshot 必须清 lock、读取新 filter 和新的 Freeze tick。
- client UI：`WornContainerPanel` 与 `PackContainerWindow` 各覆盖合法 category、非法 category、合法 prefix、非法 prefix、empty、filter 合法但 footprint 冲突六路；非法均断言 `INVALID` tint。`InspectScreen` 专属接线测试覆盖合法 category→`VALID`、合法 prefix→`VALID`、category/prefix reject、unlocked empty、footprint conflict、locked corruption、revision 替换后读取新 filter/lock；三 surface 均断言同一规则且仍发 Intent。
- locked source：Worn/Pack/Inspect 均覆盖 locked source→健康 container/hotbar/equip，UI 仍发送 Intent，server 返回同类 typed owner reject，inventory/freshness 深比较与 revision 均不变。
- 权威链：client 预判非法、locked target 或 locked source 后仍断言发出 `InventoryMoveIntent`；server 返回 `ContainerFilterRejected` 或同类 typed owner reject，且 snapshot / revision 不变。
- 九容器 e2e：逐一打开对应 owner 容器，八个非 empty 容器合法物通过、filter-invalid 物拒绝；`moisture_guard` empty 全收，负例仅 bounds/collision/full/owner corruption；另覆盖两个同模板 owner 同时存在时 UI / server 都不串 filter。
- bot 场景：新增容器 filter move 场景，至少覆盖 `ore_sack` 矿石通过、草药拒绝、swap 反向拒绝、`moisture_guard` 的非 filter 负例、locked source typed reject、拒绝后 revision/layout 不变。

## §8 开放问题（历史表，全部已收口）

| # | 历史问题 | 状态 |
|---|----------|------|
| 1 | 容器归属继续走旧嵌套/session，还是改绑当前平展模型？ | 已在 §8.1 #1 收口 |
| 2 | owner/filter 是否允许多套 resolver、缺 owner 是否 fail-open？ | 已在 §8.1 #2 收口 |
| 3 | `moisture_guard` 精确行为与 rate？ | 已在 §8.1 #3 收口 |
| 4 | 本 plan 到底交付哪些随身容器，容量/filter 如何固定？ | 已在 §8.1 #4 收口 |
| 5 | client 从哪里取得 filter 与 dragged item category，预判是否可替代 server？ | 已在 §8.1 #5 收口 |
| 6 | 剩余实施如何拆 PR？ | 已在 §8.1 #6 收口 |

> 原表仅保留作历史追溯；不存在待拍板项，实施一律以 §8.1 决议为准。

## §8.1 决议（pre-P1 收口，2026-07-24）

### #1 owner-instance 平展架构是唯一执行路线

**决议**：`PlayerInventory.containers` + `ContainerState.owner_instance_id` 为唯一状态与归属模型；稳定 `pack_<instance_id>` 只作 ID，不恢复历史方案，也没有前置 plan blocker。

**落点**：`server/src/inventory/mod.rs:469-482`（`ContainerState`）/ `server/src/inventory/mod.rs:767-772`（`PlayerInventory`）+ 本 plan「当前架构基线与范围边界」「历史否决」。

### #2 一份 resolver、typed corruption、filter OR

**决议**：`classify_container_ownership` 冻结四静态 ID allowlist（`body_pocket` / `main_pack` / `small_pouch` / `front_satchel`）及四行分类真值表；`find_live_container_owner` 是唯一 owner lookup，只查 worn、held、hotbar、`body_pocket`，排除普通静态 grid、任意 `pack_*` grid 与 owner 自身容器。P1 同 PR 以它替换 `rebuild_containers_from_equipment` live 判据，并在 owner 离开 eligible surface 时同一 prepare/commit 原子 spill/remove。runtime grant、move、swap 共用其上的 `resolve_container_acceptance`。静态带 owner=invalid，非 allowlist ownerless（未知/伪pack/placeholder）=missing，非 allowlist ownerful 只按字段查 eligible owner/template/`ContainerSpec` 并校验 `pack_<owner_id>`；None/empty 全收，非空列表 OR；任何损坏 typed reject。每个 Container from/to 端点先过结构门，源端不套 filter、目标端才套；swap 在写状态前双向校验，拒绝保持 revision/layout 原样。

**落点**：`server/src/inventory/mod.rs:83-107,6549-6592`（静态 ID 锚）/ `server/src/inventory/mod.rs:2117-2130`（现有局部 grant helper）/ `server/src/inventory/mod.rs:3828-3938`（原子 move/swap）/ `server/src/inventory/mod.rs:5572-5647`（attach 校验）+ 本 plan P1。

### #3 freshness 映射与提交边界固定

**决议**：`sealed_vial=Halve`、`spirit_seal_box=Freeze`、`moisture_guard=SpoilOnly { rate: 0.3 }`；只复用既有公式。P1 对 `prepare_inventory_mutation_with` 传 identity `InventoryItemPrepareFn`；P2 将该 prepare fn 强制替换为 `apply_container_freshness_transition(now_tick, from/to behavior)`，它在 merge 选择前操作每个 incoming staged copy，最终仅由 `commit_prepared_inventory_mutation` 一次写入 staged freshness、布局与 revision。P2 持久化 `integrity_lock(reason, detected_tick)` 并用 `reconcile_container_integrity_freshness` 统一处理首次 lock、同 reason no-op、reason change 与 repair：首次/changed/repair 各恰好 +1，active Freeze interval 保守整段 discard，repair 按 new now_tick 重开 Freeze；既有 server snapshot emit 只在 reconciliation 后读取该状态并强制 Normal，跨端 lock/reason 序列化留给 P4。

**落点**：`server/src/shelflife/container.rs:29-98`（multiplier / enter / exit）/ `server/src/spiritwood/mod.rs:611-647`（既有 behavior resolver）/ `server/src/network/client_request_handler.rs:14984-15278`（move 成功/拒绝分支）+ 本 plan P2。

### #4 恰好九个随身容器，矩阵不再可选

**决议**：交付集合与完整 `ContainerSpec`（rows、cols、weight_capacity、equip_slot、durability_cost_per_op、attrition_exempt、quick_access、accept）以 P3 九行矩阵为唯一答案；`moisture_guard` 取 empty 全收；暗器十二项全部迁 `Anqi`。owner 容器只在 worn/held/hotbar/body_pocket 位置 live，移入普通或嵌套 grid 的 rebuild 原子 spill/remove 已由 P1 交付，P3 只验证其与新数据的 save/load 接线。`coin_box` 只承诺骨币分类/收纳便利，不承诺隔灵、少漏、保鲜、保值或半衰变化；`herb_crate_placed` 为 scope-out 的独立 placeable twin。本 plan 不再使用“12 容器”口径。

**落点**：`server/assets/items/workbench_materials.toml` / `server/assets/items/anqi.toml` + 本 plan P3 与「范围边界」。

### #5 snapshot 派生 filter，client 只预提示

**决议**：P4 独占 `acceptance_lock` 的跨端契约：filter 与 `acceptance_lock` 同挂 `ContainerSnapshotV1`，item category 挂 `InventoryItemViewV1`，Rust / TypeBox / protobuf / client 同步。P4 将 P2 已持久化的 `integrity_lock.reason` 映射为 `owner_missing|owner_not_found|owner_invalid`；lock=None 时 empty 才全收，corrupt owner 的 corrective snapshot 必须携带真实 id/owner/revision/layout/items 与 `acceptance_lock` reason，不能用 empty filter 掩盖。P2 的既有 server snapshot emit 仅在 reconciliation 后强制 Normal，不负责 lock/reason 序列化。Worn/Pack/Inspect 三个 surface 共用 `ContainerFilterRules.accepts` 与 `INVALID`，locked source 或 target 均仍发 `InventoryMoveIntent`，server 先查 from/to Container 结构并 typed reject；Inspect 同时 pin 合法 category/prefix 的 `VALID` 正路径。client 永远不能吞掉请求，server 永远权威。

**落点**：`server/src/schema/inventory.rs:241-287` / `proto/bong/envelope.proto:688-695` / `agent/packages/schema/src/inventory.ts:322-363` / `client/src/main/java/com/bong/client/network/InventorySnapshotHandler.java:139-168` / `client/src/main/java/com/bong/client/inventory/component/GridSlotComponent.java:36-77` + 本 plan P4。

### #6 P1–P4 严格四 PR 串行

**决议**：P0 已是历史 merge；剩余恰好四个 PR，分别只消费 P1、P2、P3、P4。每个 PR 必须先完成仓库 review / CI / merge gate，下一 PR 才从最新 `origin/main` 开始，禁止并行跨阶段实现。P4 在代码/e2e 初步收敛并完成首轮 CI/review 后，于同一 PR 追加阶段状态、真实 `Finish Evidence` 与归档 commit；该新 HEAD 必须完整重验才可 merge，绝不另开第五 PR。

**落点**：本 plan §10。

## §10 实施工作流

本 plan 剩余 scope = P1–P4 四个严格串行 PR。纯逻辑、数据与既有 UI tint 接线，不产出 NBT、layout、bbmodel 或新贴图，因此三轮视觉资产打磨与 `<PROMISE>` 不适用。

### §10.1 串行不变量

1. PR-N 开始前 `git fetch origin`，核验上一 PR 已 merge，并以最新 `origin/main` 建独立 branch/worktree。
2. 一个 PR 只改本 plan 当前阶段，不提前夹带下一阶段，也不修改其它 plan；唯一例外是 P4 在代码/e2e 初步收敛后，于**同一 PR**追加本 plan 的状态、真实 `Finish Evidence` 与 `git mv` 归档 commit，绝不创建第五 PR 或直推 main。
3. 每个 PR 依次完成：饱和测试 → 受影响栈完整本地 gate → 合并最新 `origin/main` 后复验 → `/review` + CodeRabbit → e2e / required CI 收敛 → merge。
4. P4 的归档 commit 改变 HEAD 后，必须在该 exact HEAD 重新跑 validator、审核 Workflow、全部受影响本地门禁、CI、`/review` 与 CodeRabbit；全绿后才 merge。前一 PR 未通过 review/merge gate，后一 PR 不开工；review 返工导致 HEAD 变化同样必须重跑相应验证并重新触发 `/review`。
5. P4 merge 即代表已归档；不得在 merge 后产生归档提交、第五 PR 或直接推送 main。

### §10.2 剩余 PR 拆分

1. **PR-1（P1）预提交 owner/filter 权威门与 live rebuild**：`PreparedInventoryMutation`、`prepare_inventory_mutation_with`、`commit_prepared_inventory_mutation`、identity `InventoryItemPrepareFn`、静态 allowlist/唯一 `find_live_container_owner`、所有 Container from/to 结构门与目标 filter、runtime grant/move/swap 同门；同 PR 用 live helper 替换 rebuild，eligible→普通/嵌套/self 的原子 spill/remove 与 overflow drop outcome。仅 server 栈；完整 gate 为 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
2. **PR-2（P2）freshness prepare 与持久化损坏时钟**：`apply_container_freshness_transition` 作为 prepare fn、所有 grant 成功形态的 staged identity/merge、`ContainerState.integrity_lock` persistence、`reconcile_container_integrity_freshness` 调度/state machine、强制 Normal、既有 server snapshot emit 的 reconciliation 后状态读取、sweep/repair/revision 与守恒边界测试；不新增或序列化 `acceptance_lock` / lock reason。依赖 PR-1 merge；仅 server 栈并跑完整 server gate。
3. **PR-3（P3）九容器与暗器数据**：九容器完整 TOML `ContainerSpec`、十二暗器 category、coin_box 收纳便利文案、基于已落地 P1 live 语义的 registry/save-load/filter 矩阵测试；八个 non-empty 容器有 filter-invalid 测试，`moisture_guard` 只测 empty 全收和非 filter 负例，`herb_crate_placed` 不改。依赖 PR-2 merge；仅 server 资产与 server 测试，跑完整 server gate。
4. **PR-4（P4）wire/client/e2e 与归档**：P2 persisted integrity lock 的 reason→`ContainerAcceptanceLockV1` / snapshot `acceptance_lock` 映射 + item category、Rust/TypeBox/protobuf/client 全链、带 lock reason 的 corrective snapshot wire、Worn/Pack/Inspect 三 surface 的 locked source/target 与 VALID/INVALID 预提示、bot 与九容器 e2e。代码/e2e 初步收敛后，在同一 PR 追加阶段✅、真实 `Finish Evidence` 与归档 commit；新 HEAD 重跑 exact-HEAD validator、审核 Workflow、所有受影响门禁、CI、`/review`/CodeRabbit 后才 merge。依赖 PR-3 merge；跑 schema build/test、server 完整 gate、client `./gradlew test build` 与仓库 e2e。

### §10.3 每 PR 独立实施上下文

每个 PR 使用独立实现 agent；prompt 必须带当前阶段范围、上一 PR merge SHA、owner-instance 红线、饱和测试清单与“不得恢复历史路线”。实现 agent 只回报 PR URL、HEAD SHA、测试与 validator 结论；review 返工使用同一远端 PR 分支，禁止重复 promotion 或提前归档。

### §10.4 本 plan 的实施 agent 模型治理（2026-07-24 用户覆盖）

本节按用户 2026-07-24 的最新硬约束，覆盖 `docs/CLAUDE.md §6.4` 对本 plan 的通用模板：实施、编辑、测试、Git 与 GitHub 操作一律使用 `subagent_type: claude`、`model: sonnet`，prompt 末尾必须带 `ultrathink`。GPT/Opus 只能用于 read-only validator、对抗审核或 final judge，绝不得实施。实现 agent 只负责实施、测试、提交、push、提 PR，不等待 review 或 merge；主流程负责 ScheduleWakeup、返工调度、merge 与清理。

### §10.5 完成条件

只有 P1–P4 全部 merge、九容器 e2e 通过、跨端 symbols 对拍且 P4 同 PR 内的 `Finish Evidence` 五栏填实、归档 commit 及其新 HEAD 所有门禁/CI/review 重验全绿后，才把本 plan 视为完成。P0 的历史 merge 不能替代剩余四阶段验收。


## Finish Evidence

### 落地清单

### 关键 commit

### 测试结果

### 跨仓库核验

### 遗留 / 后续
