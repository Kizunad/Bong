# BugHunt: 满包兜底强塞分支不做网格碰撞检测，PvP 截胡/死亡转移会把多件物品叠成永久不可拾取的孤儿

## Bug 摘要

**严重度：high**

`server/src/inventory/mod.rs::force_attach_item_to_inventory`（L4733-L4764）在 `find_first_fit_container_location` 找不到空位时，会退化为“兜底强塞”分支（L4757-L4763）：直接对目标容器 `push(PlacedItemState { row: 0, col: 0, instance: item })`，**完全绕过** `attach_at_location` / `validate_attach_fits` / `placed_item_footprints_overlap` 这套全仓库统一维护的网格碰撞不变量。

进入这条分支的前提恰恰是「该容器所有格子都放不下这件物品的 footprint」——也就是说 `(row:0, col:0)` 几乎必然已经被别的合法物品占用。强塞之后，同一容器里会同时存在多个 footprint 重叠、锚点都钉在 `(0,0)` 的 `PlacedItemState`。这打破了 `inventory/mod.rs` 里所有其它写入路径（`attach_at_location` 前必经 `validate_attach_fits`、`place_item_into_container` 内联 `placed_item_footprints_overlap` 检查，甚至测试自带的 `assert_container_has_no_overlaps` 断言）共同维护的「同容器内 footprint 不重叠」不变量。

这条兜底分支目前完全没有测试覆盖：唯一调用 `transfer_all_inventory_contents` 的测试 `transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins`（mod.rs:13970 起）用的目标容器是 3x3 且只预置 1 件物品，转移 3 件必然全部走 `find_first_fit_container_location` 成功路径，兜底分支从未被真正执行过。

## 实际游玩体验影响

真实可达触发链：`tribulation_intercept_death_system`（`server/src/cultivation/tribulation.rs`）监听渡劫拦截击杀——任何在场外玩家在受害者渡劫 Lock/Wave/HeartDemon 阶段内命中并补刀击杀受害者（`record_tribulation_interceptor_system` 登记 interceptor，任意近战/技能命中即可），死亡结算会调用 `transfer_all_inventory_contents(&mut victim_inventory, &mut killer_inventory, ...)`，把受害者的容器全部物品 + 装备全层 + hotbar，逐件 `force_attach_item_to_inventory` 塞进击杀者背包。

只要击杀者自己的背包已经比较满——正常游玩中完全常见，尤其是蹲点等人渡劫的 PvP 玩家本来就爱带满装备去开荒——受害者剩余物资一旦超出击杀者剩余空位，后续物品就会命中无碰撞检测的兜底分支。结果：

- 玩家眼里看到“截胡成功、战利品到手”，但实际背包里有物品叠在同一格锚点上；
- 叠格的物品互相遮挡/覆盖，UI 上只能看到最后 push 的那一件，其余的**实质上永久不可拾取**（除非未来专门写迁移脚本扫描重叠格并重新摆位）；
- 更隐蔽的是另一条触发路径：`fauna/bone_coin.rs:208` 锻骨币产出同样调用 `force_attach_item_to_inventory`，玩家背包接近满时锻造骨币也会静默叠出孤儿物品，且没有任何错误提示——玩家会以为锻造失败或物品消失，实际是被叠死在格子里。

这类静默数据损坏对末法修士而言尤其致命：战利品/丹药/功法残页这类稀缏资源一旦被叠死，等同于凭空蒸发，且没有任何日志或反馈能定位问题——纯粹的“东西没了”。

## 证据定位

- `server/src/inventory/mod.rs:4733-4764` — `force_attach_item_to_inventory` 本体；`find_first_fit_container_location` 失败后的兜底分支，尤其 L4757-L4763 硬编码 `row:0, col:0` 的 `push`，无任何碰撞/重叠检测。
- `server/src/inventory/mod.rs:4696-4699`（`transfer_all_inventory_contents` 内）— 逐件 `for item in items { force_attach_item_to_inventory(to, item); }` 调用点，一次死亡转移可能触发多次强塞。
- `server/src/fauna/bone_coin.rs:208` — 骨币锻造产出 `output` 同样走 `force_attach_item_to_inventory(inventory, output)`，是本函数第二条生产调用路径。
- `server/src/cultivation/tribulation.rs:3673-3729`（`tribulation_intercept_death_system`），核心转移调用在 `3708-3712`：`DeathEvent` 触发后，若击杀者在 `state.participants`（截胡 interceptor 名单）中，调用 `transfer_all_inventory_contents(&mut victim_inventory, &mut killer_inventory, &item_registry)`，可达、非 dev-only、由真实 PvP 截劫机制驱动。
- `server/src/inventory/mod.rs:5572`（`validate_attach_fits`）、`:6219`（`attach_at_location`）、`:6359`（`find_first_fit_container_location`）、`:6448`（`placed_item_footprints_overlap`）— 全仓库统一的碰撞检测家族，唯独兜底分支不调用。
- `server/src/inventory/mod.rs:7006`（测试内 `assert_container_has_no_overlaps` 辅助函数，被 8880/9261 附近测试使用）— 证明“容器内不重叠”是本文件其它路径公认维护的不变量。
- `server/src/inventory/mod.rs:13970`（`transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins`）— 目标容器 3x3 只预置 1 件物品，转移 3 件，`find_first_fit_container_location` 全程成功，兜底分支无测试覆盖。
- `server/src/inventory/mod.rs:1839-1896`（`add_item_to_player_inventory_or_ground`）— 仓库已有的“容器真满 → 走地面掉落”范本：`Err starts_with("inventory full:")` 时构造 `DroppedLootEntry` 插入 `DroppedLootRegistry`，返回 `GrantOrGroundOutcome::DroppedToGround`。fix 应复用同一模式而非自造一套。
- `server/src/inventory/mod.rs:2132`（`find_free_slot`）— 已有的“容器内找空闲格”工具函数，兜底分支重新定位目标容器后可复用它而非硬编码 `(0,0)`。

## 触发路径

1. 玩家 A（受害者）进入 DuXu 渡劫（Lock/Wave/HeartDemon 任一阶段），背包内携带多件战利品/丹药/装备。
2. 玩家 B（在场外，非渡劫参与者）在此期间对玩家 A 造成命中，通过 `record_tribulation_interceptor_system` 登记为 interceptor。
3. 玩家 A 在渡劫过程中被玩家 B 击杀，触发 `DeathEvent`。
4. `tribulation_intercept_death_system` 识别到击杀者在 participants 名单中，调用 `transfer_all_inventory_contents(&victim, &killer, ...)`。
5. 该函数把受害者容器全部物品 + 装备全层 + hotbar 逐件 drain 后，循环调用 `force_attach_item_to_inventory(&killer_inventory, item)`。
6. 若玩家 B 背包已经比较满（正常端游常见），某些物品在 `find_first_fit_container_location` 扫描全部容器/全部行列后仍找不到空位。
7. 兜底分支被触发：不做任何碰撞检测，直接把物品 `push` 进目标容器，锚点写死 `(0,0)`——该位置几乎必然已被占用。
8. 玩家 B 背包内该容器出现多个 footprint 重叠、锚点相同的 `PlacedItemState`；UI 只渲染最后一个，其余物品实质丢失，且不可拾取、不可交易、不可丢弃（除非专门写工具扫描重叠格）。
9. （次要路径）玩家背包接近满时通过 `bone_coin.rs:208` 锻造骨币，产出骨币同样可能被强塞叠死，玩家无任何错误提示。

## 反方审查记录

- 第一轮质疑：
  - 质疑“进入兜底分支的前提是否真的意味着 (0,0) 已被占用”——核查 `find_first_fit_container_location`（mod.rs:6359）确认它会扫描**所有**已携带容器（含 body_pocket）的**每一行每一列**并调用 `validate_attach_fits`；只有全部候选位置都因 footprint 越界或与已有物品重叠而失败，才会 fall through 到兜底分支。因此“兜底分支被触发”与“该容器至少存在物品占据部分格子”高度相关，(0,0) 被占用是常见而非罕见情形，尤其在末期背包接近满载时。
  - 质疑“这是否只是理论可能，实战是否真能凑出这个条件”——核查 `tribulation_intercept_death_system` 的真实生产调用链（`add_systems(Update, tribulation_intercept_death_system)`），确认它由真实 PvP 机制（渡劫截胡）驱动，非 dev-only 命令。只要双方背包合起来超出击杀者容量，这个条件在正常端游中就会自然出现，不需要刻意构造的边界场景。
  - 查找是否已有 plan/PR 覆盖此问题：`docs/finished_plans/plan-bughunt-inventory-transfer-orphan-pack-v1.md` 是唯一提及 `force_attach_item_to_inventory` 的已归档 plan，但它解决的是完全不同的失败模式（drain 后 `pack_<id>` 孤儿容器壳导致 loader 判定整份 inventory 损坏、回退默认新手 loadout），修复点在 `transfer_all_inventory_contents` 末尾新增 `rebuild_containers_from_equipment(from, ...)`，只处理受害者侧，未触碰 `force_attach_item_to_inventory` 的兜底分支或击杀者侧。全仓库 grep `force_attach_item_to_inventory` 调用点只有 3 处（自身定义 + `mod.rs:4698` + `bone_coin.rs:208`），未见其它 in-flight plan（`craft-refund-full-inventory-loss`、`alchemy-takeback-full-inventory-loss`、`player-trade-npc-gate`、`dropped-loot-pickup-stack-merge`、`npc-trade-bundle-count-bridge` 等）触碰这条调用链。
  - 初裁：倾向通过。
- 第二轮补证：
  - 逐行核对 `mod.rs:4733-4764`，确认兜底分支的三步（定位 `MAIN_PACK_CONTAINER_ID` 容器 → 找不到则用任意已有容器 → 都没有则新建 16x16 容器）之后统一走同一个无检测 `push`，无论目标容器是既有的还是新建的空容器（新建空容器场景本身不会撞车，但既有容器场景是常态）。
  - 核对测试覆盖缺口：`transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins`（mod.rs:13970）目标容器 3x3 只预置 1 件物品，转移 3 件全部落在空位，`find_first_fit_container_location` 全程成功，兜底分支代码路径在现有测试套件中**从未被执行**，佐证这是真实的测试盲区而非“已知且已测试过的边界”。
  - 核对修复可行性：`add_item_to_player_inventory_or_ground`（mod.rs:1839）已提供“容器真满 → 走地面掉落 `DroppedLootRegistry`”的现成范本，`find_free_slot`（mod.rs:2132）已提供“容器内找空闲格”的现成工具，`assert_container_has_no_overlaps`（mod.rs:7006）证明“不重叠”是本文件公认不变量。修复不需要发明新机制，只需把兜底分支接入既有工具链。
  - 让步：本 finding 未新增可运行测试用例，当前为源码路径静态复现 + 现有测试盲区实证；具体断言留给 Skeleton Fix Plan 落地时补齐。
  - 终裁：通过。反方认为这是缺少碰撞检测的真实数据损坏 bug，且触发条件（击杀者背包较满）在正常末期 PvP 玩法中完全自然，不需要放宽到“扩展 inventory 系统设计”的范畴——修复应严格限定在兜底分支内部补齐碰撞检测 + 真满时转地面掉落，不触碰其它调用路径的行为。

主循环复核：已亲读关键行确认（`mod.rs:4675-4764`、`4696-4699`、`13970-14019`、`fauna/bone_coin.rs:180-219`、`cultivation/tribulation.rs:3660-3750`、`1839-1896`、`7006-7017`、辅助函数签名行 2132/5572/6219/6359/6448 均已核实存在）。

## Skeleton Fix Plan

- [ ] 在 `force_attach_item_to_inventory`（mod.rs:4733）内，`find_first_fit_container_location` 失败后，**不要**直接定位目标容器就无脑 `push`；改为：
  - [ ] 先按现有优先级（`MAIN_PACK_CONTAINER_ID` → 任意已有容器 → 新建 16x16 容器）定位候选容器；
  - [ ] 对该候选容器调用 `find_free_slot`（mod.rs:2132）**重新**尝试寻找该 item footprint 的真实空位，找到则走 `attach_at_location`（会内部 `validate_attach_fits`）正常写入，返回成功。
- [ ] 若候选容器（含新建的空 16x16 容器）依然放不下该物品的 footprint（理论上只会发生在物品本身 footprint 超过 16x16，属极端配置错误），**不允许**再退化为无检测 `push`；应视为“背包真满”，改走地面掉落分支：
  - [ ] 复用 `add_item_to_player_inventory_or_ground` 同款的 `DroppedLootRegistry` / `DroppedLootEntry` 模式，把放不下的物品判定为掉落战利品。
  - [ ] `force_attach_item_to_inventory` 需要扩展签名（或新增 `force_attach_item_to_inventory_or_ground`）以接收 `Option<&mut DroppedLootRegistry>` + 掉落所需的 `world_pos` / `dimension` 参数；两条生产调用路径需同步适配：
    - [ ] `transfer_all_inventory_contents`（mod.rs:4675 起，调用点 4696-4699）：`tribulation_intercept_death_system` 可以拿到 `death.target` 的最后位置作为掉落坐标，把放不下的战利品洒在截胡死亡现场（而不是静默叠死在击杀者背包里）。
    - [ ] `bone_coin.rs:208`：锻骨币场景放不下时，产出骨币掉落在玩家脚下，而不是静默叠死。
  - [ ] 若当前迭代暂时无法给出合理的掉落坐标上下文（如某条调用路径缺 `world_pos`），至少要让该分支**拒绝写入并返回明确错误/日志**，而不是继续无检测 push——宁可暂时“物品去向不明但有日志可查”，也不能重复当前“静默叠死不可拾取”的行为。
- [ ] 不改变 `find_first_fit_container_location` / `attach_at_location` / `validate_attach_fits` / `placed_item_footprints_overlap` 这套既有碰撞检测家族的行为——本 fix 只是让兜底分支也纳入这套体系，不新增一套平行逻辑。
- [ ] 本 bug 不涉及真元/灵气流动，无需 `qi_physics::ledger` 口径；但需注意 `bone_coin.rs:207` 附近 `cultivation.qi_current -= plan.total_qi_cost` 已发生在 `force_attach_item_to_inventory` 调用之前——若骨币产出改为可能掉落地面，真元消耗与产出物去向的时序不能颠倒（先扣真元锻造成功，产出物无论进背包还是掉地面都不应该回滚真元消耗，否则会引入新的真元路径分歧，需在实施时明确这条边界并写注释说明为何不回滚）。
- [ ] 补充线上脏数据评估（见「风险」节）：若已有玩家背包中存在历史叠格数据，本 fix 只阻止未来再产生，不会自动修复既有脏数据。

## 验收测试计划

- `server::inventory` 单测（`cargo test -p bong-server inventory::` 或对应 crate 内 `cd server && cargo test`）：
  - **happy path**：目标容器有充足空位时，`force_attach_item_to_inventory` 走 `find_first_fit_container_location` 成功路径，物品被正确 `attach_at_location`，位置合法、不重叠（复用/参考 `assert_container_has_no_overlaps` 辅助函数）。
  - **边界 — 兜底分支命中但仍有空位**：构造一个容器，主候选（`MAIN_PACK_CONTAINER_ID`）恰好满，但兜底候选容器内部实际仍有空闲格子（例如非主容器有空位、或新建的空 16x16 容器）——断言修复后的 `force_attach_item_to_inventory` 找到该空位正确落位，且 `assert_container_has_no_overlaps` 通过（不再是硬编码 `(0,0)`）。
  - **边界 — 容器真满**：构造一个 16x16 容器完全填满（或所有携带容器的所有格子都被填满到该物品 footprint 塞不下），断言 `force_attach_item_to_inventory`（或其扩展版本）走地面掉落分支：`DroppedLootRegistry` 新增对应 `DroppedLootEntry`、返回值/日志明确表明“转入地面掉落”而非静默吞掉。
  - **错误分支 — 无 `DroppedLootRegistry` 可用**：若调用方未传入 `DroppedLootRegistry`（比如某条尚未适配的调用路径），断言函数明确返回错误/记录日志，而不是继续执行无检测 `push`（对齐 `add_item_to_player_inventory_or_ground` 里 `dropped_loot: None` 分支的错误处理惯例）。
  - **状态转换 — `transfer_all_inventory_contents` 多物品混合场景**：重写/扩展 `transfer_all_contents_moves_containers_equipped_hotbar_and_bone_coins`（或新增专属测试），让目标容器**刻意接近满**、转移物品数超过剩余空位，断言：① 能放下的物品正确落位不重叠；② 放不下的物品转入地面掉落而非叠格；③ `outcome.items_moved` 统计口径清晰区分“背包接收”与“掉落地面”（如需要，扩展 `FullInventoryTransferOutcome` 字段并同步更新所有既有断言该结构体字段的测试）。
  - **回归 — 骨币锻造满包场景**：`fauna::bone_coin` 单测新增背包接近满时锻造的用例，断言产出骨币走同样的“找空位优先、真满转地面”逻辑，不再无检测叠格。
- 联调建议（可选，非本 skeleton 强制）：手动搭建“渡劫截胡 + 击杀者背包接近满”场景，人工核对击杀者背包无重叠格、地面出现掉落战利品实体。

## 风险

- **已有存档中的叠格脏数据**：本 fix 只能阻止未来再产生新的重叠 `PlacedItemState`，无法自动修复历史上已经通过这条兜底分支写入的脏数据（若曾发生过）。需要评估：
  - 是否值得写一个一次性迁移/巡检脚本，扫描所有玩家 `PlayerInventory.containers`，用 `placed_item_footprints_overlap` 检测重叠对，对发现的重叠物品重新走 `find_free_slot` 摆位或转成地面掉落补偿；
  - 若评估后认为影响面很小（该分支触发条件较少见）或历史数据无法可靠区分“兜底强塞产生的重叠”与其它成因，可以在 plan 归档时如实记录“未做历史数据清理，原因是 X”，不强行做没有把握的迁移。
- **签名扩展的连锁影响**：`force_attach_item_to_inventory` 目前是 `pub(crate)` 且只有 2 个生产调用点，扩展签名（加 `DroppedLootRegistry` 参数）风险可控，但要确认两个调用点各自能提供合理的 `world_pos`/`dimension` 上下文；如果某条路径拿不到合理坐标，不要为了凑参数瞎编坐标，应改走错误分支 + 日志，交由该调用路径的维护者后续决定掉落坐标语义。
- **不得借机改动无关的碰撞检测逻辑**：`find_first_fit_container_location` / `validate_attach_fits` / `placed_item_footprints_overlap` 本身没有问题，修复范围严格限定在兜底分支重新接入既有工具链 + 真满转地面掉落，不应顺手重构这套已稳定运行的碰撞检测家族。
- **真元时序**：`bone_coin.rs` 内真元消耗（`cultivation.qi_current -= plan.total_qi_cost`）先于产出物写入背包发生；若产出物因背包满转为地面掉落，不应回滚已扣除的真元（骨币已经锻造成功，只是暂存位置变成地面），否则会制造新的真元路径歧义，需要在代码注释里明确写清这条决策依据。
