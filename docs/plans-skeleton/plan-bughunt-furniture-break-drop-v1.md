# plan-bughunt-furniture-break-drop-v1

> Skeleton plan。只读审计产物，来源 issue #1930。

## §0 摘要

四种家具物品可以在 `block_place` 中被消费并写成自定义方块，但普通掉落映射 `block_drop_for` 没有 `BONG_SIMPLE_BED`、`BONG_MEDITATION_MAT`、`BONG_MOISTURE_BASE`、`BONG_SPIRIT_STONE_RACK` 分支。破坏系统只清方块和 `FurnitureRegistry`，因此合法放置的成品在破坏后没有返还，玩家永久损失放置时消耗的物品。

## §1 游玩影响

玩家放置床、冥想垫、防潮架或灵石架后无法搬家或回收；家具的生产材料/成品被当作空气删除。家具 registry 虽然正确移除，背包经济和基地布置却不闭环。

## §2 复现路径

1. 玩家背包有 `simple_bed` 等家具物品，通过 `block_place` 放置，物品在 `:224` 被消费并登记家具。
2. 以 Survival 完成对该自定义方块的 `DiggingEvent::Stop`。
3. `apply_block_drops` 读取当前 `BlockState`，调用 `block_drop_for`；四种 Bong 状态均落入 `_ => None`。
4. `apply_default_block_break` 随后把方块设为 AIR 并移除 registry，没有任何返还事件/物品。

## §3 今天 `origin/main` 证据

- `server/src/world/block_place.rs:217-249`：放置成功前调用 `consume_item_instance_once`，随后 `place_placeable` 写入方块/家具。
- `server/src/world/block_place.rs:572-575`：四个家具 template id 映射到四种 `BlockState::BONG_*`。
- `server/src/world/block_break.rs:45-64`：默认破坏只 `set_block(AIR)` 与 `FurnitureRegistry::remove`，没有生成掉落。
- `server/src/world/block_drop.rs:46-116`：`block_drop_for` 只列原版木石/矿物/土等映射，四个 `BONG_*` 均走 `_ => None`。
- `server/src/world/block_drop.rs:160-177,225-268`：掉落系统在默认破坏前读取状态并调用 `add_item_to_player_inventory`；`register` 的 `.before(apply_default_block_break)` 保证可接入此 funnel。
- `server/src/world/furniture.rs:193-209`：已有 `furniture_kind_for_template_id`/`furniture_kind_for_block_state`，并列出四个正式家具种类，可作为映射单一来源。

## §4 非重复比对

- `docs/finished_plans/plan-furniture-buff-v1.md` 已落地家具 registry 与 aura/移除行为，未覆盖放置成品破坏返还。
- `plan-block-break-integration-v1.md` 规划统一破坏 funnel；它的迁移阶段可承接通用 drop hook，但本骨架先锁定四种家具的缺失映射和返还契约。
- #1369 的同 tick Stop 去重属于 `plan-block-break-integration-v1` 的 hook 验收；本 issue 是单次合法破坏完全没有家具掉落，两者不合并为同一根因。

## §5 立项检查记录

- **worldview**：查 `灵龛`、`基地`、`物品`、`器物`、`资源`；命中 `docs/worldview.md §十一.安全空间` 与 §十资源匮乏，基地家具应是可管理资产，不能无提示销毁。
- **finished_plans**：查 `FurnitureKind`、`FurnitureRegistry`、`block_drop_for`、`BONG_SIMPLE_BED`；家具 buff plan 覆盖 registry/aura，不含成品掉落。
- **active plan**：查 `block_break`、`block_drop`、`furniture_kind_for_block_state`；未见 active plan 已补四种 drop 映射。
- **skeleton**：查 `furniture break`、`BONG_SIMPLE_BED`、`MeditationMat`、`drop`；无同一根因骨架，统一 funnel 只作为接入依赖。
- **reminder.md**：查 `家具`、`掉落`、`break`、`返还`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`DiggingEvent`、当前维度 `ChunkLayer` 的四种 `BONG_*` 状态、玩家 `PlayerInventory`、`ItemRegistry`、`FurnitureRegistry`。
- **Outputs**：家具模板 id 的 `add_item_to_player_inventory` receipt；随后才清方块/registry，失败时按既有掉落失败语义处理。
- **共享类型或事件**：复用 `BlockDropEntry`、`block_drop_for`、`FurnitureKind`、`furniture_kind_for_block_state`、`InventoryInstanceIdAllocator`；不新造家具物品类型。
- **server 符号**：`world::block_place::{consume_item_instance_once,place_placeable}`、`world::block_drop::{block_drop_for,apply_block_drops}`、`world::block_break::apply_default_block_break`、`world::furniture::FurnitureRegistry`。
- **agent**：无变更；家具放置/破坏和背包掉落均为 server gameplay 链路，不进入 agent IPC。
- **client**：无变更；客户端继续接收标准 block update/inventory snapshot，返还由 server authoritative inventory 处理，不改自定义 schema。
- **worldview 锚点**：`docs/worldview.md §十一` 安全空间/基地与 §十资源匮乏。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 四种家具状态到原始模板的单一掉落映射，并与 block break 顺序接通 |
| P1 | ⬜ | 满包/未知模板/重复 Stop 与 registry 一致性的回归测试 |

## P0：家具返还

- 在 `block_drop_for` 或统一 furniture drop helper 中为四种 `BONG_*` 返回对应 template id，复用 `FurnitureKind` 的映射，不能复制四套字符串常量。
- 保持 `apply_block_drops.before(apply_default_block_break)`；掉落入包成功后才允许方块清除，失败时不得静默吞掉家具。

## P1：失败和去重边界

- 覆盖每种家具、Survival Stop/Creative Start、满包、未知 template、家具 registry 缺失和同 tick 重复 Stop。
- 与统一破坏 funnel 的 #1369 验收对拍，保证同一次破坏最多返还一次。

## 验收测试计划

- 放置四种家具后破坏，各自恰好返还一个对应 template，方块变 AIR 且 registry 条目移除。
- 满包时按既有 inventory/drop policy 保留成品或生成地面掉落，不能无声丢失。
- 同 tick 重复 Stop 不重复返还；非 Survival/非完成状态不产生家具掉落。

## 来源 issue

- #1930 `[flash-review][major] 破坏家具方块不返还物品，放置消耗的成品永久丢失`
