# plan-bughunt-inventory-transfer-orphan-pack-v1

> 一句话主题：**截劫夺包复用 `transfer_all_inventory_contents` 后，受害者 inventory 会残留孤儿 `pack_<id>` 容器；该脏状态一旦 autosave / shutdown 落盘，下次登录会被 loader 当成 `#736-corrupted v2 inventory` 丢弃，并回退默认 loadout。**

> 去重说明：本条不是你点名排除的 **extra hand equip gate / TSY death drop 分流 / tool weapon HUD leak / BaiYanPeng 引怪漂移**，也不是最近文档里已立项的 container filter / nested pack / dead armor contamination 题。核心缺口落在 **inventory 全量转移 helper 没有维护 `pack_<id>` 派生容器不变量**。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 截劫夺包后受害者落盘为孤儿 `pack_<id>` 脏档，重登回退默认 loadout | fix_pr | ✅ 2026-07-12 |

## P0 — 截劫夺包后受害者落盘为孤儿 `pack_<id>` 脏档

- **复现路径（真实玩法链，不是 dev-only 命令）**
  1. 玩家 A 穿着任意带 `container_spec` 的背包件，使运行时存在 `pack_<instance_id>` 容器。
  2. 玩家 A 开渡虚劫；玩家 B 作为参与者截劫并击杀 A。
  3. `server/src/cultivation/tribulation.rs:3692-3698` 的 `tribulation_intercept_death_system` 对 `[death.target, killer_entity]` 调 `transfer_all_inventory_contents(&mut victim_inventory, &mut killer_inventory)`。
  4. `server/src/inventory/mod.rs:4088-4118` 的 `transfer_all_inventory_contents` 只做三件事：drain `from.containers[*].items`、`from.equipped.drain()`、`from.hotbar.take()`，然后 `bump_revision(from)`；**它没有**调用 `rebuild_containers_from_equipment`，也没有移除空的 `pack_<id>` 容器、重建 `body_pocket`/`max_weight`。
  5. 由于 `from` 的 `PlayerInventory` 被标记 changed，`server/src/player/mod.rs:754-760` `flush_changed_player_inventories` 会把这个脏 inventory 立刻 `save_player_inventory_slice(..., Some(player_inventory))` 落盘；`server/src/player/state.rs:2339-2344` 会直接把当前 `PlayerInventory` JSON 写进 sqlite。
  6. 下次玩家 A 登录时，`server/src/player/state.rs:1224-1247` 反序列化该 v2 inventory 后调用 `inventory_has_orphan_pack_container`；只要还残留 `pack_<id>` 且 owner 背包件已经不在 worn/held/hotbar/body_pocket，就命中 `return Ok(None)`。
  7. `server/src/player/mod.rs:206-253` 读取到 `persisted.inventory == None` 后不会插入 inventory；紧接着 `server/src/inventory/mod.rs:1118-1133` `attach_inventory_to_joined_clients` 会给“没有 PlayerInventory 的 joined client”实例化默认 loadout。

- **根因链路**
  - `transfer_all_inventory_contents` 假设“把物品本体 drain 走”就等于 inventory 被清空；这对静态 `main_pack` 成立，但对运行时派生的 `pack_<id>` 不成立。
  - `pack_<id>` 的存在条件不是“里面有物品”，而是“owner 背包件仍在合法携带面”。这一不变量由 `rebuild_containers_from_equipment` 维护（见 `server/src/inventory/mod.rs:4283-4393`），但 `transfer_all_inventory_contents` 绕过了它。
  - loader 明确把“`pack_<id>` 容器存在，但 owner 背包件不在 live set”定义为污染指纹（`server/src/player/state.rs:1138-1167` + `1235-1247`），且**不关心该容器是否为空**。
  - 因而截劫夺包会把受害者变成“源码层合法、持久化层非法”的中间态：在线时看起来只是被抢空；一旦 flush/rejoin，就会被持久化自愈逻辑重置成 fresh loadout。

- **影响面**
  - 直接命中生产调用面：`tribulation_intercept_death_system` 是当前唯一生产 caller（`server/src/cultivation/tribulation.rs:3692-3698`）。
  - 间接影响持久化：`flush_changed_player_inventories` 的 changed-inventory 即时刷盘和 `flush_connected_players_on_shutdown` 都会把该脏状态固化。
  - 影响玩家状态而不只影响显示：重登后不是“看到一个空背包 UI”，而是**整套 inventory slice 被丢弃，换成默认新手 loadout**。
  - 现有测试没锁住这条 side path：`server/src/inventory/mod.rs:11324-11336` 只测了普通 `main_pack` + `main_hand` + hotbar 的 transfer，断言“items 被搬走、equipped 变空”；**没有**任何 `pack_<id>` / orphan / relog fallback 覆盖。

- **这个 bug 对实际游玩体验的影响**
  - 玩家在被截劫击杀后，短期体感是“东西被对方抢走了”；但只要随后重登或服务器在 autosave / shutdown 后重启，自己的 inventory 会被系统判成污染档并回退默认新手装。
  - 这会造成两种割裂：一是**受害者被抢空后反而凭空拿回默认草包/铁剑等 starter gear**，形成不符合玩法预期的白嫖补给；二是 inventory slice 被整体重置，但角色其它 slice（位置、寿元、修为等）仍沿用旧档，形成“人还是那个高周目角色，背包却突然 fresh join”的断层。
  - 对截劫玩法来说，这不是单纯的 loot 转移问题，而是**劫后 inventory 状态机被破坏**：PVP 胜者拿到战利品，败者下线再上线还会触发额外 inventory reset，既破坏一致性，也给重复薅 starter loadout 留口子。

- **修复建议**
  - 最小修：`transfer_all_inventory_contents` 结束前对 `from` 显式调用一次 `rebuild_containers_from_equipment(from, registry)` 或抽一个“full-strip inventory 后自愈派生容器”的 helper，清掉孤儿 `pack_<id>`、补齐 `body_pocket`、重算 `max_weight`。
  - 更稳妥：把所有“全量 drain inventory”路径（至少 `transfer_all_inventory_contents`、`apply_termination_drop_on_terminate`、其它 strip-all helper）统一收口到同一 invariant-restoration seam，避免再次出现“源码态可运行、持久化态被判污染”的分叉。
  - 回归测试至少补三条：
    1. 截劫 transfer 后 `from` 不得残留任何 owner 不在 live set 的 `pack_<id>`。
    2. transfer 后立即 `save_player_inventory_slice` → `load_player_inventory_from_sqlite`，不得返回 `None`。
    3. `attach_player_state_to_joined_clients` + `attach_inventory_to_joined_clients` 组合下，截劫受害者重登后不得被错误发放默认 loadout。

## 反方裁决

> 退化说明：本会话没有可再开的 subagent / delegate 接口；以下两轮反方裁决为**主会话自审退化版**，已如实记录。

### Round 1

- **反方论点**：`transfer_all_inventory_contents` 搬空了所有 `container.items`，残留的 `pack_<id>` 只是空壳；空壳既不可访问，也不应算真实 bug。
- **驳回理由**：
  - `inventory_has_orphan_pack_container`（`server/src/player/state.rs:1138-1167`）只按 `container.id` 是否是 `pack_<id>` 且 owner instance 是否仍在 live set 判定，**完全不检查 `items.is_empty()`**。
  - `load_player_inventory_from_sqlite`（`1235-1247`）对命中的 orphan 直接 `return Ok(None)`，其语义是“丢弃整份 inventory slice 回落默认 loadout”，不是“忽略空壳容器继续用旧 inventory”。
  - 所以“只是空壳、无实际后果”的前提不成立。

### Round 2

- **反方论点**：即便 loader 会把它判成 `None`，玩家也可能只是短暂没有 inventory，不一定真的拿到默认装备；因此实际游玩影响可能被夸大了。
- **驳回理由**：
  - `attach_player_state_to_joined_clients`（`server/src/player/mod.rs:243-253`）只有在 `persisted.inventory` 为 `Some` 时才插入 `PlayerInventory`。
  - 玩家实体因此会落入 `JoinedClientsWithoutInventoryFilter`，随后 `attach_inventory_to_joined_clients`（`server/src/inventory/mod.rs:1118-1133`）必定实例化 `DefaultLoadout` 并插入新 `PlayerInventory`。
  - 这不是“临时无背包”或“UI 空白”问题，而是**确定性的 default loadout fallback**。

## 审计来源

- bughunt 2026-07-05，范围聚焦 inventory / container / equipment side paths。
- 证据链仅来自仓库现状静态核对：`tribulation_intercept_death_system` → `transfer_all_inventory_contents` → immediate inventory flush → orphan-pack loader guard → joined-client default loadout attach。
- report-only：本 skeleton 不含任何源码修改建议的落地实现。
