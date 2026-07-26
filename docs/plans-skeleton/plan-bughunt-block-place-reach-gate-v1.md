# BugHunt: 方块放置 C2S 全程无 reach 距离校验，可任意远距离放置方块/容器

## Bug 摘要

**severity: high**（skeptic 未调整，`severity_adjust: unchanged`）

`server/src/world/block_place.rs::handle_block_place_requests`（配合其唯一的合法性校验函数 `can_place_block`）是全仓通用的方块放置 C2S 权威处理路径，覆盖 workbench / 储物箱（`StorageCrate`）/ 死信箱（`DeadDrop`）/ 普通 vanilla 直通方块（`torch_item`/`door_bolt`/... 及 `vanilla:<id>` 前缀直通）的放置。它从 `client_request_handler.rs` 拿到 client 发来的原始 `x/y/z` 后，**全程没有玩家到放置点的距离（reach）校验**——只要目标 chunk 已加载、目标方块可替换、且不与玩家自身碰撞盒相交，任意坐标都会被接受。同一代码库内其它放置类 C2S（棺椁 `coffin_target_is_close`、容器开箱 `OPEN_RANGE_BLOCKS`）都强制了 4-6 格量级的距离门槛，唯独这条最通用的方块放置路径完全空缺，是明显的权威校验遗漏而非有意为之的设计。

## 实际游玩体验影响

玩家背包里只要有一个可放置方块物品（`item_instance_id` 校验存在即可，不看玩家实际站位），就能对着**任何已被服务器加载过的坐标**（自己或其他玩家去过的任意区域）发送放置请求并成功落地，完全无视正常拾取/交互距离。这在多人场景下是实打实的 griefing 面：

- 远程在出生点、他人灵龛周边、阵眼区堆放死信箱/储物箱封路封门（`docs/finished_plans/plan-block-lifecycle-v1.md` §11 #4 已明确点出这个风险类别，但当时只标注为"已知缺口，范围与数值待定"，从未真正落地）；
- 在无法到达的悬空/地底/他人领地坐标"隔空"放置容器方块，造成后续拾取/清理困难；
- 削弱"放置=你人在现场"这一最基本的空间沉浸假设，末法残土的资源采集/建造玩法失去物理可信度。

## 证据定位

- `server/src/network/client_request_handler.rs:1005-1031`：`ClientRequestV1::BlockPlace { x, y, z, item_instance_id, target_face, .. }` 分支把 client 发来的 `x/y/z` 原样打包进 `BlockPlaceRequest` 并 `send`，中间没有任何距离/维度校验。
- `server/src/world/block_place.rs:103-230`（`handle_block_place_requests`）：
  - L134：`let pos = BlockPos::new(req.x, req.y, req.z);` 直接采信 client 坐标。
  - L135：取到 `player_position`，但其**唯一**用途是 L184 传给 `can_place_block` 做玩家自身碰撞盒判定，从未与 `pos` 做距离比较。
  - L146-230：后续校验依次是 `block_place_target_for_request`（物品是否可放置）、`DimensionLayers` 是否存在、`can_place_block`（见下）、容器位重复占用、`consume_item_instance_once`——**没有一步是 reach 校验**。
- `server/src/world/block_place.rs:576-598`（`can_place_block`）：函数体只做四件事——Y 边界检查、chunk 是否已加载（`ChunkNotLoaded`）、目标方块是否可替换（`TargetNotReplaceable`）、目标方块是否与玩家自身碰撞盒相交（`PlayerCollision`，`block_cell_intersects_player`，L615 起）。**函数签名里的 `player_pos: DVec3` 从未用于 `pos` 与 `player_pos` 的距离数学**，仅用于自碰撞判定。
- 同库对照（证明「通用放置路径无 reach」是异常而非全局设计）：
  - `server/src/world/container_open.rs:26-27`：`const OPEN_RANGE_BLOCKS: f64 = 4.0;` + `const OPEN_RANGE_TOLERANCE: f64 = 0.5;`，容器开箱强制距离门槛。
  - `server/src/coffin/mod.rs:100`：`const COFFIN_INTERACT_MAX_DISTANCE_SQ: f64 = 36.0;`（即 6.0 格）；`server/src/coffin/mod.rs:1217-1224`（`coffin_target_is_close`）在放置/进入/破坏/回收共 4 处调用点（`L449/609/743/875`）强制近距校验。
- `docs/finished_plans/plan-block-lifecycle-v1.md:164`（§11 开放问题 #4）：「多人 griefing 治理：全仓 0 个 build/place 保护……放置打开后任何人可在出生点/他人灵龛旁/阵眼区堆方块封门。最小 spawn-zone 保护 + reach 距离校验的范围与数值待定，**显式标注为已知缺口**。」——证明团队早已知晓这个缺口的存在，但从未在任何后续 plan/PR 里落地。
- 测试侧：`server/src/world/block_place.rs` 内 `mod tests`（`L639` 起，约 35 条 `#[test]`）覆盖了物品映射、维度选层、容器占用等分支，但没有任何一条对远距离放置做断言——默认测试玩家位置固定在 `Position::new([3.5, 64.0, 3.5])`（`L2415`），放置目标也都在 `(1,64,1)` 附近的近距坐标，从未验证过远距离场景。

## 触发路径

1. 玩家背包持有任意可放置方块物品（如 `earth_crumb`、`torch_item`、`herb_crate_placed`、`vanilla:stone` 等），不要求玩家站在目标附近。
2. 玩家（或经修改/脚本化的 Fabric 客户端）发送 `ClientRequestV1::BlockPlace { x, y, z, item_instance_id, target_face }`，坐标可以是玩家当前位置数百格外的任意已加载 chunk 内坐标。
3. `client_request_handler.rs` 原样转发为 `BlockPlaceRequest` 事件，无过滤。
4. `handle_block_place_requests` 依次校验物品合法性 / 维度层 / `can_place_block`（Y 边界、chunk 加载、可替换性、自碰撞）——全部通过，因为这些检查都与"玩家离目标多远"无关。
5. 物品被 `consume_item_instance_once` 消耗，方块/容器在远处坐标成功落地。

## 反方审查记录

- 第一轮质疑：
  - 复核 `can_place_block` 是否隐含距离逻辑（例如通过 chunk-loaded 间接限制范围）：不成立——"chunk 已加载"只要求该区域被任何人访问过，不代表当前玩家在附近。
  - 检查是否存在客户端侧的隐式限制掩盖了服务端缺口：owo-lib UI/普通客户端确实有本地射线距离限制，但服务端作为权威方从未校验，一旦客户端被修改/脚本化即可绕过——这正是「server gate 是最终权威」原则要求堵住的口子。
  - 检查是否与已知同类模块（`zhenfa` 的 `DISARM_RANGE`/`sense_range`）功能重叠导致重复报告：不重叠，`zhenfa` 的距离门是拆除/感知类判定，不覆盖通用 `block_place.rs` 路径。
  - 查开放 PR/skeleton 是否已覆盖：`docs/plans-skeleton/` 与 `docs/plan-*.md` 中未见任何以 `server/src/world/block_place.rs` 为目标文件的在跑任务；`zhenfa-place-scope-gate`、`lingtian-c2s-range-gate` 等命名指向的是阵法/灵田模块的独立范围校验，不是本文件。
  - 初裁：倾向通过。
- 第二轮补证：
  - 补充 `docs/finished_plans/plan-block-lifecycle-v1.md` §11 #4 的既有记录——团队在 2026-06-10 前后已明确知道这是"已知缺口"，当时判断非 P0 阻塞、范围与数值待定，此后再无 plan/commit 跟进补上。这说明本 finding 不是新发现的设计分歧，而是长期悬空的已知 TODO，具备直接立 plan 收口的正当性。
  - 补充同库两处已有先例（`container_open.rs` 的 `OPEN_RANGE_BLOCKS = 4.0`、`coffin::mod.rs` 的 `COFFIN_INTERACT_MAX_DISTANCE_SQ = 36.0` 即 6.0 格）作为数值量级参照，避免修复时凭空定数字。
  - 让步：本发现是静态代码路径复现（未在真实客户端上实际发包验证 exploit），但代码逻辑链路是确定性的——`can_place_block` 函数体逐行读完确认无任何距离数学，非推测。
  - 终裁：通过。反方认为这是缺少权威 server 侧 reach gate，属于收敛式最小修复（补一条距离校验），不需要引入新的空间保护系统（spawn-zone 保护是 plan-block-lifecycle-v1 §11 里的另一个更大的开放问题，超出本 bug 的最小修复范围，不应该在本 plan 里顺手做）。

主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 在 `server/src/world/block_place.rs` 顶部新增常量 `const PLACE_REACH_BLOCKS: f64 = 6.0;`（对齐仓库内 `coffin::COFFIN_INTERACT_MAX_DISTANCE_SQ`=36.0 即 6.0 格量级，比 `container_open::OPEN_RANGE_BLOCKS`=4.0 略宽松以覆盖持有长柄工具/隔着一格墙放置的常规体验；具体数值在实施阶段可与 vanilla 4.5-6 格惯例再核对一次并在 commit 里写明依据）。
- [ ] 在 `can_place_block` 函数签名或调用点补一条距离校验：计算 `pos` 中心点 `(x+0.5, y+0.5, z+0.5)` 与 `player_pos` 的 `distance_squared`，超过 `PLACE_REACH_BLOCKS.powi(2)` 时返回新增的 `BlockPlaceRejectReason::TooFar`（或在 `handle_block_place_requests` 内 `can_place_block` 调用前单独判定，二选一皆可，但必须在 `consume_item_instance_once`（L220）**之前**拒绝——修复点放在消耗之后只会把"无效放置"变成"吞道具但拒绝"，是更差的回归）。
- [ ] `BlockPlaceRejectReason` 新增 `TooFar` 变体，`impl fmt::Display` 补对应分支（参照现有 `PlayerCollision`/`ChunkNotLoaded` 写法保持统一日志风格）。
- [ ] `handle_block_place_requests` 里 `can_place_block` 校验失败分支（L184-192）已有统一的 `tracing::warn!` + `continue` 逻辑，新分支自动复用，无需额外处理逻辑，只需确认新 reject reason 走的是同一条日志/拒绝路径。
- [ ] 明确本次修复**只补服务端权威 reach gate**，不新增客户端 UI 隐藏/提示（若客户端已有本地射线限制则保持不变，纯粹是 UX 增强，不能替代 server 校验；server 拒绝必须在客户端毫无预判的情况下依然生效）。
- [ ] 不在本 plan 内展开 `docs/finished_plans/plan-block-lifecycle-v1.md` §11 #4 提到的更大范围"spawn-zone 保护"系统——那是独立的空间保护设计问题，本 plan 严格收敛为"放置必须在合理距离内"这一条最小修复，避免范围蔓延。
- [ ] 本修复不涉及真元/灵气流动（`block_place.rs` 全程无 `qi_current`/`spirit_qi` 读写），无需 `qi_physics::ledger` 接线；仅涉及 C2S 权威校验，遵循「server gate 是最终权威，client 隐藏只是 UX」原则。
- [ ] 修复完成后同步检查 `docs/finished_plans/plan-block-lifecycle-v1.md` §11 #4 的措辞是否需要补一条"reach 校验已在 plan-bughunt-block-place-reach-gate-v1 收口"的引用注记（若归档流程允许，作为本 plan 的 Finish Evidence 交叉引用，不改该 finished plan 本体的阶段状态）。

## 验收测试计划

全部在 `server/` 用 `cargo test` 跑（Rust 单测，复用 `block_place.rs` 已有的 `block_place_app` / `inventory_with_item` / `item_instance` / `inventory_template_count` / `block_state_at` 测试 helper，测试玩家默认坐标 `Position::new([3.5, 64.0, 3.5])`）：

- **happy path（边界内应放置成功，防回归）**：玩家在默认坐标附近（如 `(4, 64, 4)`，距离约 0.7 格）发送 `BlockPlaceRequest`，断言方块成功写入（`block_state_at` 命中预期 `BlockState`）且物品被消耗（`inventory_template_count` 减少）。
- **边界 1（恰好等于 `PLACE_REACH_BLOCKS`）**：目标坐标距玩家恰好 `PLACE_REACH_BLOCKS` 格（如取整数格心距离精确等于门槛），断言放置成功（`<=` 语义下边界包含）。
- **边界 2（略超出 `PLACE_REACH_BLOCKS`）**：目标坐标距玩家 `PLACE_REACH_BLOCKS + 0.1` 格，断言放置被拒绝：
  - `block_state_at` 目标坐标仍为 `BlockState::AIR`（未写入）；
  - `inventory_template_count` 物品数量不变（未消耗，对齐 `handler_rejects_missing_instance_without_consuming_or_writing` 等既有测试的断言范式）；
  - 无对应容器/家具组件被 spawn（`StorageCrate`/`DeadDrop` 场景另起一条断言 `ExternalContainerRegistry`/`ContainerBlock` 查询为空）。
- **错误分支（极端远距离）**：目标坐标距玩家数百格外（如 `(500, 64, 500)`，属于"任何人访问过的已加载 chunk"场景），断言同上——拒绝、不消耗、不写入，锁死本次 bug 报告里描述的具体 exploit 路径。
- **状态转换 1（拒绝原因优先级）**：构造一个同时"距离超限"且"目标是他人碰撞盒内"的 case，断言拒绝原因是可预期的（无论 `TooFar` 判定顺序放在 `PlayerCollision` 前后，测试要断言实际生效的拒绝路径，不留歧义）。
- **状态转换 2（跨维度不受影响）**：复用 `handler_selects_layer_from_player_dimension` 的模式，在 TSY 维度里同样验证近距放置成功、远距放置拒绝——确认 reach gate 与维度选层逻辑正交，不会因为维度分支漏掉校验。
- **回归**：跑一遍现有 `block_place.rs` 内全部既有测试（约 35 条），确认新增距离校验不破坏任何近距 happy-path 测试（现有测试坐标集中在默认玩家位置附近的 `(1,64,1)` 类坐标，理论上都在新阈值内，但必须实测确认无一条因新增校验意外变红）。
- 跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全量门禁，确认新增 `BlockPlaceRejectReason::TooFar` 分支没有引入 clippy 警告（如 match 穷尽性、未使用变体等）。

## 风险

- 数值拍板：`PLACE_REACH_BLOCKS` 具体取值（4.0 / 5.0 / 6.0）需要在实施时与 vanilla Minecraft ~4.5-6 格放置距离惯例、以及仓库内 `OPEN_RANGE_BLOCKS=4.0`/`COFFIN_INTERACT_MAX_DISTANCE_SQ`(6.0 格) 两个既有先例对齐，避免定得过紧导致正常楼梯/隔墙放置体验受损，或过松导致 gate 形同虚设。
- 拒绝时机：修复点必须卡在 `consume_item_instance_once`（L220）之前。若不慎放在消耗之后，会把"未授权的远距放置"改造成"扣道具但放置失败"的更差回归，等同于新增一个吞物品 bug。
- 范围蔓延风险：`docs/finished_plans/plan-block-lifecycle-v1.md` §11 #4 里提到的"spawn-zone 保护"是比 reach gate 大得多的独立设计问题（涉及区域权限、他人领地判定等），本 plan 严禁顺手把两者混在一起实现，否则会把一个小修复膨胀成一个新功能 plan，违反最小正确修复原则。
- 线上已有数据：若已有玩家利用此漏洞在远处放置了容器/方块，修复 gate 不会自动清除已放置的方块——这属于运营侧清理范畴，不在本 plan 修复范围内，仅在此风险项中提示知悉。
