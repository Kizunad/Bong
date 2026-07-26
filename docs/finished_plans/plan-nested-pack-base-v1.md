# plan-nested-pack-base-v1 — 塔科夫式套包系统基础（物品内嵌子容器 + 重量向上累加）

> **归档（被取代 / withdrawn）2026-07-26**：本 plan 的 P0-P5 **从未按本文档实施**。全部 6 个核心协议 symbol（`ItemInstance.sub_container` / `MAX_PACK_NEST_DEPTH` / `PackItemSession` / `PackContainerOpen`/`Move`/`Close` C2S 变体 / `SubContainerPanel.java`）在生产代码里**零命中**（证据见下方「替代关系与代码现实」）。与此同时，`plan-tarkov-backpack-v1`（`docs/finished_plans/plan-tarkov-backpack-v1.md`，✅ 2026-06-27 全阶段归档）沿着 `plan-layered-equip-v1` → `plan-backpack-equip-v1` 的既有穿戴槽血统，用完全不同的一套协议（`ContainerState.owner_instance_id` + 平展 `PlayerInventory.containers` + `pack_<instance_id>` 命名 + `InventoryMoveIntent` + `WornContainerPanel`/`PackContainerWindow`）交付了等价甚至更完整的「塔科夫式套包」玩家体验（连货整体卸下 / 双击开包 / 重量递归上卷 / 上身渲染 / 悬浮多窗）。**本 plan 提出的 session-based 嵌套子容器协议是被否决路线，不再是任何后续实施的依据；禁止独立消费。**
>
> 详见下方「替代关系与代码现实」「未覆盖能力与后续归属」「Finish Evidence」三节。原始设计正文（接入面 / P0-P5 详细交付物 / §8 开放问题 / §8.1 决议 / §10 实施工作流）已整体移除，完整旧草案由 **git 历史**保留（`git log --follow docs/plan-nested-pack-base-v1.md`，骨架首次提交见 `d74ddd379`/`575207504`，升 active 见 `7c3656758` 之前的版本历史）。

## 阶段总览（历史记录，均未按本文档实施）

| 阶段 | 原定内容 | 状态 |
|------|---------|------|
| P0 | 浮窗 spike + `ItemInstance.sub_container: Option<ContainerState>` 数据模型 + `MAX_PACK_NEST_DEPTH` 嵌套护栏 + 向后兼容序列化 | ⬜ WITHDRAWN |
| P1 | 重量递归累加（`instance_total_weight`）+ 死亡掉落递归展平（三路径） | ⬜ WITHDRAWN |
| P2 | `PackContainerOpen/Move/Close` schema + `PackItemSession` + 临时容器注入/写回 | ⬜ WITHDRAWN |
| P3 | `validate_move_semantics` 套包语义 + `build_inventory_snapshot` 递归 emit + 持久化 round-trip | ⬜ WITHDRAWN |
| P4 | client `SubContainerPanel.java`（root 顶层可拖拽浮窗）+ 双向 drag 路由 + 视听 | ⬜ WITHDRAWN |
| P5 | 5 随身子包 TOML 升级（`herb_pouch`/`projectile_bag`/`ore_sack`/`water_skin`/`herb_crate`）+ 端到端验收 | ⬜ WITHDRAWN |

> 原表标注「⬜」是 plan 自身草案状态（从未被任何后续 commit 推进）；本次归档额外加 `WITHDRAWN` 标记，表明该阶段**不会**再被实施——需求已被 `plan-tarkov-backpack-v1` 用另一套协议满足。

---

## 替代关系与代码现实（2026-07-26 实地验真，worktree HEAD `52892ed22c5acf0c418f2914fae72726d4bfd032`）

### A. 本 plan 提出的旧协议 symbol —— 生产代码零命中

对 `server/src`、`client`、`agent` 三个目录做全量文本检索（ASCII symbol，grep 直接可靠），结果：

| Symbol | 命中 |
|--------|------|
| `sub_container`（`ItemInstance.sub_container`） | 无命中 |
| `MAX_PACK_NEST_DEPTH` | 无命中 |
| `PackItemSession` | 无命中 |
| `PackContainerOpen` | 无命中 |
| `PackContainerMove` | 无命中 |
| `PackContainerClose` | 无命中 |
| `SubContainerPanel` | 无命中 |

即：本 plan 设想的「物品实例内嵌 `Option<ContainerState>` 子容器 + session-scoped 临时容器注入 + 独立 C2S 三变体 + 独立浮窗面板类」这一整套协议，**从未落地一行代码**。§8.1 #7 定案的「S2C 复用 `LootContainerOpenV1`」也因为整条 C2S/session 链路都不存在而失去意义。

### B. 当前权威底盘 —— 真实生产代码

穿戴式套包能力实际由以下模块交付，均可在当前 HEAD 命中：

- **`ContainerState.owner_instance_id: Option<u64>`**（`server/src/inventory/mod.rs:482`，`ContainerState` struct 定义于 `:469`）——字段注释显式标注来源 `plan-tarkov-backpack-v1 P0（决议 #1，方案 A）`：容器归属的穿戴背包件 instance_id，`serde(default)` 容旧档，load 时按 `pack_` 前缀回填。
- **`PlayerInventory.containers: Vec<ContainerState>`**（`server/src/inventory/mod.rs:769`，struct 定义 `:767`）——平展设计，容器不挂在物品实例内部，而是与物品实例平级存在 `PlayerInventory` 的一个 `Vec` 里，靠 `owner_instance_id` 反向关联穿戴背包件。这与本 plan「物品实例内嵌 `sub_container` 字段」的树状设计是**根本不同的数据形状**。
- **`pack_<instance_id>` 命名规则**：`container_id_for_worn_pack`（`server/src/inventory/mod.rs:4794`）+ 反解 `worn_pack_instance_from_container_id`（`:4799`），注释标注来源 `plan-layered-equip-v1 P0.2（决议 #17）`——命名规则的血统比本 plan 更早、来自不同的 plan 谱系。
- **`rebuild_containers_from_equipment`**（`server/src/inventory/mod.rs:4887`）——按穿戴背包件动态重建/刷新 `pack_<id>` 容器，取代了本 plan P2 设想的「open 时懒初始化 + 临时容器注入 + close 时写回」session 机制。
- **`InventoryMoveIntent`**（C2S，定义于 `server/src/schema/client_request.rs:335`）——server 端消费于 `server/src/network/client_request_handler.rs`、`server/src/inventory/mod.rs`；client 端发送方在 `client/src/main/java/com/bong/client/inventory/WornContainerPanel.java`。本 plan P2 设想的 `PackContainerMove` 独立 C2S 变体从未存在，跨包/包内移动统一走这一个既有 intent。
- **`InventorySnapshotV1`**（server 定义 `server/src/schema/inventory.rs:268`，emit 于 `server/src/network/inventory_snapshot_emit.rs`，agent 侧镜像 `agent/packages/schema/src/inventory.ts`）——取代本 plan P3 设想的「`build_inventory_snapshot` 递归 emit `pack_{instance_id}` 容器」，实际做法是平展 snapshot 本就包含所有 `ContainerState`（含 `pack_<id>` 派生容器），无需递归。
- **`WornContainerPanel`**（`client/src/main/java/com/bong/client/inventory/WornContainerPanel.java:43`）+ **`PackContainerWindow`**（`client/src/main/java/com/bong/client/inventory/component/PackContainerWindow.java:32`，`extends DraggableContainer<FlowLayout>`）——取代本 plan P4 设想的「从零搭建 `SubContainerPanel` 可拖拽浮窗」。`PackContainerWindow` 是「悬浮容器多窗系统」（见下方 commit `c4f431176`），比本 plan P4 设想的单浮窗方案更完整（支持多窗同开）。

**结论**：旧 session-based 嵌套子容器 wire 协议是**被否决路线**——不是「还没做」，是「换了一条完全不同的数据形状（平展 + `owner_instance_id` 反向关联）做出来了同等能力」，不作为遗留继续实现，不得被后续 plan 复用或引用其 symbol 名。

---

## 未覆盖能力与后续归属

本 plan 原 scope 里有几项 `plan-tarkov-backpack-v1` 未覆盖、仍需下游认领的能力：

1. **随身子包容器化**（`herb_pouch` / `projectile_bag` / `ore_sack` / `herb_crate` 等从 `category = "misc"` 升级为 `category = "container"` + filter / freshness 行为）——**归属 `docs/plan-container-filter-and-completion-v1.md`**（当前为 active plan，其 P3 阶段「anqi category 迁移 + 随身子包闭环 + filter 落地」覆盖 `herb_pouch`/`ore_sack`/`projectile_bag`/`water_skin`/`herb_crate` 等一批随身容器的 category 升级 + `accept` filter）。
   - **需要人工核实的遗留问题**：`plan-container-filter-and-completion-v1.md` 头部（L4/L22/L35/L37/L74/L133/L185/L192-193）目前显式依赖本 plan 提供的 `sub_container` / `PackContainerOpen/Move/Close` / `container_id = "pack_{instance_id}"` / `SubContainerPanel`，并写着「本 plan 全部 merge 到 main 后才开」。这些引用的 symbol 现已证实**从未存在**，该下游 plan 的接入面描述已经与代码现实脱节，需要独立任务去核对并重写其依赖引用（改指向 `owner_instance_id`/`WornContainerPanel`/`PackContainerWindow` 一线）——**本次归档范围明确不包含改动 `plan-container-filter-and-completion-v1.md`**，仅在此记录留痕。
2. **可拖拽 root overlay 浮窗基建**（本 plan 原 P4 §8 #5 点名的最高风险项）——若后续仍有需要独立可拖拽浮窗（非套包场景）的 UI 需求，`PackContainerWindow`（`extends DraggableContainer<FlowLayout>`）已提供了可复用的拖拽窗口基类，**不需要另起 plan 从零验证可行性**；若有新场景需要则应在对应 UI plan 里直接引用 `DraggableContainer`，不得再假称「塔科夫已覆盖」等价于「所有浮窗需求已覆盖」。
3. **`water_skin` 灌装实装**——已移出套包 scope，归属 `docs/plans-skeleton/plan-satiety-hydration-v1.md:114`（饱食度/水分双轴生存系统骨架，2026-07-18 已立骨架，尚未升 active）。该骨架文档已明确记录 `water_skin_filled` 需要从零接线（TOML 注册 + 消费链 + icon），不存在捷径。
4. **worldview §九:808 转移税**（「inventory 操作扣灵气纯度 1-5%」）——**已在 `docs/plans-skeleton/reminder.md:31` 找到对应条目**：「§808 转移税另立 plan 待办（2026-06-10 甩锅消解后遗留）」，明写两个套包族 plan 均已划出 scope，另立 plan 时需先扩 `qi_physics` 定义扣减率常数 + ledger 归还路径，不可自拍 1-5% 数值。该 reminder 条目继续由 `reminder.md` 管理，本次归档不改动它。

---

## Finish Evidence

> 本节口径是 **supersession closure**（治理归档：确认旧协议未落地 + 指向真实替代模块），**不是 feature completion**——本 plan 的功能性交付物从未实施，不存在可核验的"完成"。

- **落地清单**：本次归档动作仅涉及 `docs/plan-nested-pack-base-v1.md` → `docs/finished_plans/plan-nested-pack-base-v1.md` 的治理性 `git mv` + 内容改写，不涉及任何代码模块落地。当前真实替代模块路径：
  - `server/src/inventory/mod.rs`（`ContainerState.owner_instance_id:482`、`PlayerInventory.containers:769`、`container_id_for_worn_pack:4794`、`rebuild_containers_from_equipment:4887`）
  - `server/src/schema/client_request.rs:335`（`InventoryMoveIntent`）、`server/src/schema/inventory.rs:268`（`InventorySnapshotV1`）
  - `client/src/main/java/com/bong/client/inventory/WornContainerPanel.java:43`、`client/src/main/java/com/bong/client/inventory/component/PackContainerWindow.java:32`
- **关键 commit**（`plan-tarkov-backpack-v1` 谱系，`git log` 逐一核实，非编造）：
  - `4b255485a` 2026-06-26 — P0：`owner_instance_id` + 移除非空拒卸 + rebuild/overflow→掉落接进 move 路径（#760）
  - `efa6772d9` 2026-06-26 — P1：重量递归上卷 pin 测试（#762）
  - `7a153389a` 2026-06-27 — P2：跨包/包内移动穿戴态门控 + schema 漂移修复（#763）
  - `6f9bb2ceb` 2026-06-27 — P3：双击打开穿戴背包件容器视图 + `owner_instance_id` 全栈下发（#765）
  - `c53d43eaa` 2026-06-27 — P4：破草包上身渲染 TPV（#773）
  - `25edd4a9d` 2026-06-27 — P5：套包操作差异化视听 + 2 层封顶固化 + 平衡标定 + 归档（#774，`plan-tarkov-backpack-v1` 本身此时归档进 `docs/finished_plans/`）
  - `2d06bc08d` 2026-06-28 — 后续修复：套包随包保留容器 + 任意位置双击/右键开包（#777）
  - `c4f431176` 2026-06-28 — 后续增强：套包悬浮容器多窗系统（`PackContainerWindow`）+ 去 tab + 拖入门控对齐（#778）
  - `pack_<instance_id>` 命名规则本身来自更早的 `plan-layered-equip-v1`（决议 #17），未逐一列 commit hash——见该 plan 自身 `docs/finished_plans/plan-layered-equip-v1.md` 的 Finish Evidence。
- **测试结果**：本 PR 为 **docs-only**（仅 `git mv` + 文本改写一个 plan 文件），未跑 `cargo test` / `./gradlew test` / `npm test` 任何命令，无新增/变更代码故无需跑。真实替代模块（`plan-tarkov-backpack-v1`）的测试结果见其自身 Finish Evidence（`docs/finished_plans/plan-tarkov-backpack-v1.md`），本文档不重复转录、不伪造数字。
- **跨仓库核验**：
  - **负向命中（本 plan 旧协议，2026-07-26 实测）**：`sub_container` / `MAX_PACK_NEST_DEPTH` / `PackItemSession` / `PackContainerOpen` / `PackContainerMove` / `PackContainerClose` / `SubContainerPanel` —— 在 `server/src`、`client`、`agent` 全部**无命中**。
  - **正向命中（真实替代协议，2026-07-26 实测，file:line 见上方「替代关系与代码现实」）**：server `ContainerState.owner_instance_id`（`inventory/mod.rs:482`）、`PlayerInventory.containers`（`:769`）、`container_id_for_worn_pack`（`:4794`）、`rebuild_containers_from_equipment`（`:4887`）；schema `InventoryMoveIntent`（`schema/client_request.rs:335`，另命中 `network/client_request_handler.rs`）、`InventorySnapshotV1`（`schema/inventory.rs:268`，另命中 `network/inventory_snapshot_emit.rs`、`network/agent_bridge.rs`、`agent/packages/schema/src/inventory.ts` 等）；client `WornContainerPanel.java:43`、`PackContainerWindow.java:32`。
- **遗留 / 后续**：
  1. 随身子包容器化（`herb_pouch`/`projectile_bag`/`ore_sack`/`herb_crate` 等）→ 归 `docs/plan-container-filter-and-completion-v1.md`（active，P3 阶段），但该 plan 头部依赖引用已指向本 plan 从未存在的 symbol，**需要独立任务核对重写**（本次归档范围不含该改动）。
  2. 可拖拽 root overlay 浮窗基建 → `PackContainerWindow`（`extends DraggableContainer<FlowLayout>`）已提供可复用基类，新场景直接复用，不必另立 spike plan。
  3. `water_skin` 灌装实装 → 归 `docs/plans-skeleton/plan-satiety-hydration-v1.md:114`（骨架，未升 active）。
  4. worldview §九:808 转移税 → 见 `docs/plans-skeleton/reminder.md:31`，继续由该 reminder 条目管理，另立 plan 时需先扩 `qi_physics`。
