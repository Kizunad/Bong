# plan-bonecoin-wallet-bridge-v1 — 骨币双轨归一：物理骨币成为唯一支付介质，legacy 标量钱包退役

> **一句话主题**：搜刮/击杀获得的**物理骨币 item**（`bone_coin_5/15/40`、`fengling_bone_coin`，每枚带 `spirit_quality`）与 NPC 交易/灵田实际扣费的 **legacy 标量钱包** `PlayerInventory.bone_coins` 是两套互不相通的账——标量钱包在初始 7 枚后几乎无自然进账，**交易经济整体饿死**；且标量"按枚计价"直接违反 worldview 红线「骨币价值按剩余真元，不按枚数」。本 plan 把所有支付路径切到物理骨币（按 `face_value × spirit_quality` 计真元价值），标量钱包彻底退役，不留兼容层。
>
> 来源：2026-07-18 早期玩法诊断——「搜刮→变现」断链两环之一（搜到的钱花不出去）。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 支付原语：物理骨币凑值/找零 + 面值统一 helper 对齐 | ⬜ |
| P1 | NPC 交易切换（报价语义真元化 + client 价签/支付预览 UI） | ⬜ |
| P2 | 杂项 sink 切换（灵田补灵/阵法欺天等）+ 标量钱包字段退役 | ⬜ |
| P3 | 支付流向守恒收口 + bot e2e + 经济回归 | ⬜ |

## 现状证据（2026-07-18 Explore 实证）

- **双轨并行**：`economy/mod.rs:80` 世界骨币供给统计里 `.saturating_add(inventory.bone_coins)`——legacy 标量与物理币派生值在同一 supply struct 并列入账，但**玩法层零转换通道**。
- **支付全走标量**：NPC 交易 `client_request_handler.rs:1557`（`inventory.bone_coins < price` 拒付）/ `:1594`（扣减）；报价字段 `offer.price_bone_coins: u32` 按枚计价（`:1467`）；灵田补灵 `lingtian/systems.rs:546,1301-1302` 按枚扣 1。
- **标量进账死水**：初始 loadout 7 枚（`assets/inventory/loadouts/default.toml:11`）、遗骸拾取转移（`inventory/mod.rs:1148`，死亡时 `:1000-1062` 灌入遗骸）、dev 命令——**loot/掉落/制作全部产出物理币 item**（`loot_pools.json`、`fauna/drop.rs`、`fauna/bone_coin.rs` 制作），进不了钱包。
- **物理币现有唯一 sink**：手搓当材料（`craft/mod.rs` axe_bone 吃 3 枚 `bone_coin_5`）。
- **正典冲突**：worldview §九 + `plan-economy-v1` §0「骨币不是堆叠数字：每枚都有独立真元残量」+ journey §K 红线「不把骨币写成稳定货币：价值按剩余真元，不按枚数」——标量钱包本身就是违典遗留。

## 为什么不并入既有 plan（§四红旗自查）

- [[plan-bughunt-bonecoin-qi-facevalue]]（skeleton）修的是 **qi_physics 记账口径**（磨损/守恒快照未乘面值），本 plan 修的是**玩法支付通道**——两者共享「面值 × spirit_quality」语义，P0 的统一 helper 必须与其 `item_effective_qi` 收敛为同一个（协调点见 §8 #1），但修复面不重叠。
- `plan-economy-v1`（finished）价格指数已按 `face_value × spirit_quality` 聚合供给——本 plan 是把**支付侧**对齐它已定的口径，不改价格公式。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`PlayerInventory` 物理骨币 item（`bone_coin_5/15/40` 模板 + `spirit_quality`）；`economy` 面值表（`economy/mod.rs:59-64`）与价格指数（`estimate_item_price_for_index`）；[[plan-bughunt-bonecoin-qi-facevalue]] 的 `item_effective_qi` helper（若其先落地则直接复用）。
- **出料**：`NpcTradeRequest` / 灵田 / 阵法欺天等全部 sink 改吃物理币；被支付骨币的去向（NPC inventory / 世界回收，§8 #3）；client 交易 UI 价签真元化。
- **共享类型 / event**：不新增货币类型；`price_bone_coins` 字段语义迁移（枚 → 真元价值，wire 变更连 proto/samples 一起改，不做 dual-form）。
- **跨仓库契约**：server schema `client_request.rs` / `server_data.rs` 交易族 payload 字段语义 + proto + `agent/packages/schema` samples 双端；client `TradeOffer` 屏价签与支付预览。
- **worldview 锚点**：§九 经济（唯一真货币骨币 / 价值按真元）；§K 红线「不把骨币写成稳定货币」；`world-0004 骨币半衰录`（半衰后同一价签需要更多低灵币——玩法自然涌现，不加补偿机制）。
- **qi_physics 锚点**：支付本身是物品实例转移，不产销 qi（骨币封存真元随 item 走）；**任何"支付即销毁"路径是守恒红旗**——销毁必须走 `qi_release_to_zone` 把封存真元还给 zone；骨币衰变继续走既有 `qi_excretion(ContainerKind::SealedInBone)`，本 plan 零新常数。

## P0 — 支付原语 ⬜

- `inventory::bone_coin_payment`：`quote_payment(inventory, price_qi) -> PaymentPlan`（从背包物理骨币贪心凑 `face_value × spirit_quality` 总值 ≥ 价格）+ `apply_payment(plan)`（移除/部分消耗选中币）。
- 找零策略 §8 #2 拍板（推荐：**不找零、按最接近超付成交**——末法没有精确找零的银行，超付是残酷世界的一部分；client 支付预览明示超付额）。
- 面值语义统一：与 [[plan-bughunt-bonecoin-qi-facevalue]] 共用同一 `item_effective_qi`（放 `inventory`/item 语义层，避免 qi_physics 反向依赖 economy——该 skeleton 风险节已提示的依赖方向问题在此一并解决）。
- 测试：凑值边界（不足/刚好/超付/全零灵币/混合面值/堆叠）、贪心策略 pin（优先花低灵币还是高灵币——§8 #2 连带拍板）、`apply_payment` 原子性（失败不动背包）。

## P1 — NPC 交易切换 ⬜

- `offer.price_bone_coins` 语义迁 **真元价值**（定价源已是 `estimate_item_price_for_index` 的真元口径，改动集中在扣费侧 `:1557-1594` 换 `bone_coin_payment`）；schema/proto/samples 同步，旧字段名如语义不清则改名（干净代码约定，不留 dual-form）。
- client：交易屏价签「值 X 真元」；支付预览面板列出将消耗的骨币（枚数 + 各自残量 + 超付额）；余额显示从标量数字改为「背包骨币总真元」。
- 测试：买/卖双向、余额不足拒付、超付成交、交易后双方物品/骨币守恒；schema 正反 sample 对拍。

## P2 — 杂项 sink 切换 + 标量退役 ⬜

- 灵田补灵（`lingtian/systems.rs:546,1301`）、阵法欺天成本等逐个切 `bone_coin_payment`（全仓 grep `bone_coins` 清单化，逐处迁移或删除）。
- `PlayerInventory.bone_coins` 字段删除：死亡遗骸链（`inventory/mod.rs:1000-1062,1148`）改走物理币 item 掉落；`economy/mod.rs:80` 供给统计删标量项；持久化迁移——存量玩家钱包余额一次性铸成等值物理币（`spirit_quality=1.0` 的 `bone_coin_5` 若干，铸造走既有 `bone_coin` 模板，**封入真元从哪来见 §8 #4 守恒拍板**）。
- 测试：全仓 `bone_coins` 符号零残留（编译即证）；迁移往返（旧存档载入 → 物理币到账 → 再存不回退）。

## P3 — 支付流向守恒收口 + bot e2e ⬜

- **支付去向拍板落地**（§8 #3）：付给 NPC 的骨币不得蒸发——推荐进 NPC 自己的 `PlayerInventory`（与 [[plan-npc-combat-gear-v2]] B 路线天然合流：NPC 死后可被搜刮回来，形成"买了他的货、抢回我的钱"的末法闭环）；灵田/阵法类非 NPC sink 的骨币走 `qi_release_to_zone`（封存真元还地脉）+ 骨壳销毁。
- bot 场景 `economy_bone_coin_trade.py`：搜刮得物理币 → NPC 交易扣真实 item → 断言背包变化 + NPC 侧到账/守恒；灵田补灵场景改走物理币断言。
- 经济回归：价格指数在双轨归一后的供给口径核验（`summarize` 只剩物理币项）。

## §8 开放问题（升 active / P0 决策门前收口）

1. **与 bonecoin-qi-facevalue 的落地顺序**：两 skeleton 共享 `item_effective_qi`——先修记账口径再切支付（推荐，helper 由它立、本 plan 复用）vs 本 plan 先行自立 helper 它来对齐。两 PR 不得各写一份（近义重名红旗）。
2. **找零与凑币策略**：不找零超付成交（推荐）vs 拆分 spirit_quality 找零（等于发明"分币"，违背每枚独立语义，不推荐）；凑币顺序优先花低灵币（玩家利益直觉）vs 高灵币（防囤积，配合半衰押注）。
3. **NPC 收款去向**：入 NPC PlayerInventory（推荐，依赖 npc-combat-gear-v2 P1 落地节奏）vs 世界回收池过渡方案——若 gear-v2 未先行，P1 是否临时走"回收池"再迁（倾向：等 gear-v2 P1，避免过渡态）。
4. **存量标量余额铸币的守恒来源**：迁移铸出的物理币封存真元从哪记账（一次性从 `WorldQiBudget` 沉降槽划拨 vs 铸 `spirit_quality=0` 空壳币只保面值）——空壳币违背"价值按真元"，划拨需 qi_physics 侧确认口径；数额极小（每人 ≤7 枚）但守恒律无小事。
5. **`price_bone_coins` wire 字段处置**：原地改语义（枚→真元）vs 改名 `price_qi_value`——倾向改名自documenting，破坏面已由 proto/samples 同步覆盖。

## 现状证据补充（2026-09-13，P2-43 搬迁期实测）

> 本节只记录本轮对现有代码的实测，不把疑点直接定性为已违反既定契约；本 skeleton 尚未被消费。NPC 无限库存或货币 sink 也可能是有意的虚拟商店设计。证据的用途是给 P1/P3 提供精确的接入起点。

### NPC 买货路径

- `server/src/network/client_request/npc.rs:223-227` 从目标 NPC 的 `NpcTradeInventory.offers` 找到匹配项并 `.cloned()`，这里只读取 offer 的 `template_id`、`count`、`price_bone_coins` 等字段，没有写回 offer、NPC 库存组件或 NPC 物品容器。
- `server/src/network/client_request/npc.rs:294-305` 读取玩家 `PlayerInventory`，以 legacy 标量 `inventory.bone_coins` 检查余额；`price` 也仍由 `price_bone_coins` 按枚数语义计算。
- `server/src/network/client_request/npc.rs:311-318` 调用 `add_item_to_player_inventory(..., template_id, offer.count, tick)`。该 helper 的入口在 `server/src/inventory/mod.rs:1840-1857`，内部先在 `:1993-1999` 查模板，再由模板建立/合并运行时物品；这里发给玩家的是模板物化的 `ItemInstance`，不是从 NPC 持有的 `ItemInstance` 转移出来的实例。
- 成功发货后 `server/src/network/client_request/npc.rs:322` 只执行 `inventory.bone_coins = inventory.bone_coins.saturating_sub(price)`；随后 `:327-329` 发送“从 NPC 手中买下”的消息。该路径没有 NPC 钱包入账、NPC 物品扣减、世界回收账户入账或物理骨币 item 转移。

### 双轨表示与供给投影

- legacy 钱包字段是 `server/src/inventory/mod.rs:775-782` 的 `PlayerInventory.bone_coins`，并投影到 `server/src/schema/inventory.rs:266-277`。遗骸流程在 `server/src/inventory/mod.rs:1027-1028` 将其移出玩家，在 `:1194-1201` 拾取加回；通用 inventory transfer 在 `:4758-4762` 也按标量做 from→to 搬运。
- 物理骨币由 `server/src/inventory/mod.rs:510-521` 的 `ItemInstance` 承载，模板 ID 与每实例 `spirit_quality` 同时存在；`server/src/economy/mod.rs:59-70` 识别面值和 rotten 状态，`server/src/economy/mod.rs:221-235` 将 `face_value × spirit_quality × stack` 计入物理币的 `active_coin_count`、`total_face_value`、`total_spirit_qi`。
- `server/src/economy/mod.rs:73-85` 另把 `inventory.bone_coins` 累加到 `BoneCoinSupply.legacy_scalar_count`，没有换算进 `total_spirit_qi`；因此同一 supply struct 内是两条并列账，当前没有玩法层转换通道。
- IPC 仍保留双轨：`server/src/schema/economy.rs:14-24` 的 `BoneCoinTickV1` 同时暴露 `total_spirit_qi` 与 `legacy_scalar_count`；`server/src/schema/economy.rs:66-78` 的 `PriceIndexV1.supply_spirit_qi` 来自物理币 `total_spirit_qi`，不包含 legacy 标量换算值。

### `add_item_to_player_inventory` 生产调用面

当前 `server/src` 中，除测试代码外，已核到的调用点和可读出的来源语义如下。这里仅区分“是否存在事件、材料消费、返还或 dev/tutorial 前置来源”，不声称表中其他路径已经满足完整的骨币/物品守恒契约。

| 调用点 | 当前用途/来源判断 |
|---|---|
| `network/client_request/npc.rs:311` | NPC 交易出货；无 NPC `ItemInstance` 前置转移，最直接的模板物化 + legacy 标量扣款疑点。 |
| `botany/harvest.rs:1899` | 采集结果入玩家包，来源是 harvest session 的植物产出。 |
| `lingtian/systems.rs:1465,1495` | 灵田收获物与种子奖励，来源是已完成的 harvest session。 |
| `world/block_drop.rs:241` | 方块掉落，来源是 block drop 事件。 |
| `craft/workbench.rs:239` | 工作台拆除返还，来源是已存在的工作台物品。 |
| `coffin/mod.rs:541` | 棺材位置登记失败时返还此前已接受/登记失败的物品。 |
| `coffin/mod.rs:1112` | 棺材 reclaim drop 返还已登记的回收掉落。 |
| `combat/tuike.rs:349` | 退魄/伪皮制作产物，调用前已有材料消费和 staged inventory。 |
| `network/craft_emit.rs:586` | 制作完成产物，调用前在 staged inventory 中消费材料。 |
| `network/tuike_ash_emit.rs:42` | 灰烬回收产物，来源是 ash event 的 `output_item_id`。 |
| `zhenfa/mod.rs:3461` | 阵法拆解/解除后的 pearl 奖励，来源是已处理的阵法事件。 |
| `world/spawn_tutorial.rs:381,646` | 新手教程固定赠品，属于显式的一次性 tutorial grant。 |
| `cmd/dev/give.rs:102`、`cmd/dev/block_picker.rs:87` | dev-only 物品发放，属于明确的测试/开发入口。 |
| `inventory/mod.rs:1840` | 统一 grant primitive 本身，不是独立的 gameplay 来源。 |

`server/src/npc/loot.rs:533`、`coffin/mod.rs:2359,2499,3145` 以及 `server/src/network/client_request_handler_tests.rs:6143` 等命中位于 `#[cfg(test)]` 测试代码，未计入上述生产影响面。该分类只说明 NPC 路径缺少可见的前置物品来源，不能替代后续对各自契约的独立守恒核验。

### 归属与后续边界

本证据属于既有 `plan-bonecoin-wallet-bridge-v1` 的 P1/P3 范围，不新建 plan，也不修改 P2-43 搬迁涉及的生产文件。后续实现仍需在 P1 收口 NPC 收款方和物品所有权，在 P3 决定非 NPC sink 的回收路径，并用 e2e 证明支付失败原子性及支付后去向；本节仅提供上述 file:line 起点。
