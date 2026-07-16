# plan-bughunt-spiritwood-full-inventory-loss-v1

> **Finished Plan（2026-07-15 archived；2026-07-15 promotion）**。来源：
> `docs/plans-skeleton/plan-bughunt-spiritwood-full-inventory-loss-v1.md`。
> 一句话主题：把灵木采伐完成改成“入包或原地掉落成功后才消耗世界资源”的原子结算，禁止满包时吞掉稀缺 `ling_mu_gun` 却仍回报完成。

## 阶段总览

| 阶段 | 目标 | 状态 |
|---|---|---|
| P0 | 第一性原理复现满包吞产物，并锁定失败前不可提交世界状态 | ✅ 2026-07-15 |
| P1 | 复用 inventory-or-ground grant，保留 freshness、位置与维度 | ✅ 2026-07-15 |
| P2 | 原子提交 harvested/AIR/session/反馈并覆盖所有分支 | ✅ 2026-07-15 |
| P3 | 完成 server 全量门禁、主线同步、归档与 PR gates | ✅ 2026-07-15 |

## Bug 摘要

`complete_spiritwood_sessions` 当前先从 `WoodSessionStore` 移除完成 session、把对应 log 记入 `SpiritWoodHarvestedLogs` 并将世界方块置为 AIR，之后才尝试把 `ling_mu_gun` 发给玩家。玩家随身容器没有 `1x2` 空位、且已有堆叠因 freshness 不同不能合并时，grant 返回 `inventory full`；生产路径只写 warn，仍发送 `GatheringCompleteEvent` 与 `LumberTerminalEvent { completed: true }`。结果是原木、采伐时间和灵木产物同时丢失。

这不是纯 UI 误报。`ling_mu_gun` 是稀缺灵木资源链入口和高级真元载体，见 `worldview.md §四 L408-L412`、`§十 L893`；同类 botany 收获已经采用“入包或掉地成功后再提交 harvested”的原子口径，灵木链路没有接入。

## 接入面

- **进料**：`WoodSessionStore` 的 completed session、玩家 `PlayerInventory`、`ItemRegistry`、`DecayProfileRegistry`、`InventoryInstanceIdAllocator`。
- **出料**：优先写入玩家随身容器；仅在 `inventory full:` 时写入 `DroppedLootRegistry`，成功后才更新 `SpiritWoodHarvestedLogs`、`ChunkLayer` AIR 与采集终态事件。
- **共享类型 / event**：复用 `GrantOrGroundOutcome`、`DroppedLootEntry`、`GatheringCompleteEvent`、`LumberTerminalEvent`，不新造第二套掉落或采集事件。
- **跨仓库契约**：server 的 `InventoryItemView` protobuf 新增 optional freshness tag 22，Python Bot 镜像六个 freshness 字段与既有 `lumber_progress`；Bot 通过真实 `C2S_PLAYER_ACTION` 触发生产 `DiggingEvent`。client、agent/schema、Redis key 不变。
- **worldview 锚点**：`worldview.md §四 L408-L412` 明确灵木是优良真元载体；`§十 L893` 明确灵木是器修/暗器流核心耗材。
- **qi_physics 锚点**：本 plan 只改变同一个新产物实例的交付位置，不新增、衰减或销毁真元，不引入物理常数。掉落实例必须保留 `spirit_quality` 与 freshness，禁止通过重建缺字段实例吞掉载体属性。

## 实际游玩体验影响

玩家正常砍一棵 SpiritWood 巨树，240 tick 结束时如果随身包和身袋没有可放 `1x2` 物品的位置，会看到“采得灵木原木 ×N”，但背包里没有产物、地上也没有掉落；原木已变 AIR 且被记为 harvested，无法重试。修复后满包会在原木位置生成同维度、带完整 freshness 的地面掉落，并明确提示“背包已满，灵木原木已落地”；结构性配置错误不会消耗世界原木或伪装成功。

## 根因证据

- `server/src/spiritwood/mod.rs::complete_spiritwood_sessions`：`store.remove`、`mark_harvested`、`set_block(AIR)` 发生在 grant 之前。
- `server/src/spiritwood/mod.rs::grant_ling_mu_gun_to_inventory`：只调用 `add_customized_item_to_player_inventory`，满包错误原样返回。
- `server/src/inventory/mod.rs::add_item_to_player_inventory_or_ground`：已经实现“正常 grant；仅 inventory full 时创建 `DroppedLootEntry`”，并支持 `customize_instance`。
- `server/src/inventory/mod.rs::stack_identity_matches`：freshness 属于完整堆叠 identity；不同采伐 tick 的灵木不保证能并堆。
- `server/src/botany/harvest.rs`：相邻正确范式先完成 `Granted` / `DroppedToGround`，再提交 harvested 状态。

## 触发路径

1. 生存玩家使用合格斧头完成 SpiritWood log 的 240 tick 采伐。
2. 玩家所有随身容器没有 `1x2` 空位；旧 `ling_mu_gun` freshness 与本次新实例不同。
3. `complete_spiritwood_sessions` 先移除 session、记录 harvested 并置 AIR。
4. `add_customized_item_to_player_inventory` 返回 `inventory full: ling_mu_gun`。
5. handler 只 warn，仍发送 completed=true；产物既不入包也不落地，原木无法重采。

## 去重与范围

- 不重复灵木关服前强制 flush：后者防重启复刷，本题修在线完成时的不可逆产物丢失。
- 不重复 botany/craft/alchemy 满包题：各自生产链不同，本题只修改 `server/src/spiritwood/` 与必要 inventory helper 使用点。
- 不处理灵木跨维 session、客户端 UI 或新的采集 VFX/SFX；为黑盒回归只补生产五 tile 灵木 fixture 与 e2e 运行态持久化隔离，不改正式 worldgen 配置。
- 不修改通用 `add_item_to_player_inventory_or_ground` 的既有结构错误口径，避免影响其它生产链。

## 实施决议（2026-07-15）

1. `grant_ling_mu_gun_to_inventory` 改为接收当前 log 的 `world_pos`、`DimensionKind` 与可选 `DroppedLootRegistry`，返回 `GrantOrGroundOutcome`；freshness customization 在入包和落地两条路径使用同一闭包。
2. 满包必须整体创建一个 `DroppedLootEntry`，位置使用 `block_origin(session.log_pos)`，维度使用 `session.dimension`，数量保持 `ling_mu_drop_count`，不得拆分或丢字段。
3. 只有 `Granted` 或 `DroppedToGround` 后才执行 `mark_harvested`、`set_block(AIR)`，并发送 `GatheringCompleteEvent` 与 completed=true terminal。
4. 结构性错误不标记 harvested、不置 AIR、不发 gathering complete；本次 session 结束并发送 interrupted/completed=false，原木保持可重新采集，避免 completed session 每 tick 自动重试刷日志。
5. 入包成功沿用“采得灵木原木 ×N”；满包落地使用“背包已满，灵木原木已落地 ×N”；失败文案明确“灵木产物结算失败，原木未消耗”。

## 实施范围

### P0 - 证真与失败测试

**状态：✅ 2026-07-15**

- 增加满包、freshness mismatch 和结构性错误测试，在生产修复前证明满包不会产生地面掉落且世界状态会被错误提交。
- 测试锁玩家可观察契约，不以私有调用次数或源码字符串替代行为。

### P1 - 入包或落地

**状态：✅ 2026-07-15**

- 让灵木 grant 复用 `add_item_to_player_inventory_or_ground`。
- 断言 Granted 与 DroppedToGround 均保留 template、stack count、freshness profile、created tick、位置和维度。
- unknown template、缺 `DroppedLootRegistry`、allocator 失败保持显式 Err，不静默伪装落地。

### P2 - 原子世界提交与诚实反馈

**状态：✅ 2026-07-15**

- 重排 `complete_spiritwood_sessions`：grant outcome 成功后才提交 session/harvested/AIR/完成事件。
- 覆盖成功入包、满包落地、freshness mismatch、结构失败、无 inventory/player 等边界。
- 保持采集 quality、工具识别、drop count 与既有正常路径不变。

### P3 - 门禁与归档

**状态：✅ 2026-07-15**

- 定向运行 `cargo test spiritwood::` 及新增契约测试。
- 运行 `production_spiritwood_full_inventory_drop.py`：从生产五 tile fixture 的固定外缘主干发出真实 digging packet，验证满包落地、freshness wire 六字段与清包后同实例拾回。
- 运行 server 完整门禁：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 运行 Python protocol、完整 Bot e2e 与 JDK 17 client build，覆盖本次协议和真实玩家链路。
- fetch 后分类同步最新 `origin/main`，带入变化则重跑受影响门禁；填写 `## Finish Evidence` 并归档。

## 验收测试矩阵

- `full_inventory_drops_fresh_ling_mu_at_log_position`：满包无空位时生成一条同维度掉落，数量与 freshness 完整，inventory 不增物。
- `freshness_mismatch_does_not_merge_and_still_drops`：旧堆 template 相同但 freshness 不同，不误合并，仍落地。
- `successful_grant_commits_world_state_once`：正常入包后 session 清除、harvested 标记、log 置 AIR、完成事件各发生一次。
- `dropped_grant_commits_world_state_once_with_honest_terminal`：满包落地同样只提交一次，terminal 明确已落地。
- `structural_grant_error_preserves_log_and_reports_incomplete`：unknown template/缺 registry 等错误不标记 harvested、不置 AIR、不发完成事件，terminal completed=false。
- `normal_inventory_grant_preserves_existing_behavior`：有空间时仍入随身容器，不额外生成掉落，质量与工具反馈不回退。

## 风险与遗留

- `DroppedLootRegistry` 的实例 ID 必须来自同一 allocator；碰撞错误不得提交 harvested。
- 掉落实例必须沿用 customization，否则 freshness/载体属性会丢失。
- 世界方块层暂不可用时，既有语义只记录 harvested；本 plan 不扩展为 chunk 写失败事务。
- 不新增交互式 `runClient` 要求；生产 Bot 场景直接验证 server 启动、协议、采伐、落地同步与拾取消费链。

## 对抗审查记录

Round 1 核查了满包是否可达、灵木是否总能堆叠、inventory helper 是否已有落地兜底、世界资源是否在 grant 前消耗。结论：满包明确返回错误；freshness 使旧堆不保证可合并；灵木路径未调用已有落地 helper；生产顺序确实先提交不可逆副作用。

Round 2 排除了灵木 shutdown flush、botany/craft/alchemy 满包修复和跨维 session 等重复范围。局部原子结算可复用既有 `GrantOrGroundOutcome`，但生产黑盒回归揭示 `InventoryItemView` protobuf 未镜像 freshness，因此补齐既有协议类型而不新造状态机。

Round 3 根据 `/review` 的 major findings 复核 freshness 缺省语义、真集成覆盖、raster schema 与 session 原子顺序：registry/profile 缺失改为结构失败并保留原木；生产五 tile fixture 补齐与正式 worldgen 一致的 `0..4` biome palette，并逐 tile 锁定 `biome_id.bin` 长度与 palette 边界；`grant_before_removing_session` 保证成功与失败 grant 执行期间 completed session 均仍在 store，grant 返回后才结束 session；真实 `C2S_PLAYER_ACTION` digging、掉落同步与拾回对拍均通过。

Round 4 复核 `/review` 对 `freshness.initial_qi` 精度的质疑：权威 `Freshness.initial_qi` 在 `server/src/shelflife/types.rs` 中是 `f32`，protobuf `float` 与 Python `<f` 解码不会继续收窄。Rust 与 Python 协议测试改用非平凡 bit pattern `0x42C50001`，逐 bit 锁定 server protobuf roundtrip 和 Bot 掉落解码保真；不把 wire 无故扩成与领域类型不一致的 `double`。

## 最新增量审查证据（代码基线 `88e0c5c37d2d2d30451c3796810a4d78548ae3d8`）

最终 review 发现并已修复一个并发边界：结算消费者不能只按 player identity 删除 session。`WoodSessionStore` 现在为每个 player 保存 `settling` claim；`claim_for_settlement` 只允许一个消费者认领同一完整 session，grant 执行期间 session 仍可见，`finish_settlement` 仅在完整 identity（session id、log position、dimension、started/last tick 等）相同的情况下删除。替换 session 不会被旧结算流程误删，普通 `remove` 会清除 claim；新增测试覆盖重复 claim、grant 期间可见、replacement identity 和 mismatch 保留。

本次审查还核验了两个容易被截断误判的范围事实：`server/src/spiritwood/persistence.rs` 只增加 `BONG_SPIRITWOOD_HARVESTED_PATH` 的 e2e 路径 override；未设置或空值仍使用生产默认 `data/spiritwood/harvested.json`，并有正反测试。`server/src/world/terrain/mega_tree.rs` 只抽取既有 seed 计算并增加真实树定位 helper，供 dev `tptree` 与生产树 fixture 使用；正式 profile、seed 与 placement 配置均未改变。

当前基线本地门禁：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、完整 `cargo test` 全部通过（lib `11710 passed / 0 failed / 1 ignored`，CLI `11 passed`，full-app startup `1 passed`，背包 e2e `4 passed`，doc-tests `0 failed`）。代码基线 `88e0c5c3` 的同 SHA GitHub e2e run `29462708855` 已成功；补齐审查证据的 doc-only HEAD `33c12282` 对应 run `29463076031` 也已成功（job `87510463791`，22m46s），两次跨栈门禁均覆盖 client、schema、agent、server、Smoke/E2E、Bot e2e 与证据上传。

## 当前审查证据包（前一代码基线 `2cb2cfd078af744b40af4c4690499f0bee99c7e1`）

Review 面板的 initial prompt 无工具且 diff 上限为 40,000 字符，核心 Rust 文件会被前置文件截断；以下是可逐项对拍的生产控制流和联合终态：

- `complete_spiritwood_sessions` 调用 `grant_before_removing_session`；闭包内先调用 `grant_ling_mu_gun`，只有 `Ok(Granted|DroppedToGround)` 才执行 `mark_harvested`、`BlockState::AIR` 和 `GatheringCompleteEvent`，随后发送 `LumberTerminalEvent(completed=true)`。
- `Err` 分支只发送 `LumberTerminalEvent(completed=false, "灵木产物结算失败，原木未消耗")`，不调用 harvested、AIR、完成事件，也不新增库存或地面掉落。
- `grant_before_removing_session` 先 clone `store.session_for(player)`，执行 grant 后才 `store.remove(player)`；成功和失败 grant 的测试都在闭包内断言 session 仍可见。
- `grant_ling_mu_gun` 先取得 `DecayProfileRegistry` 与 `ling_mu_gun_v1` profile，再构造 `Freshness`，最后统一调用 `add_item_to_player_inventory_or_ground`；缺 registry/profile、缺 inventory、缺 dropped-loot registry 或实例 ID 冲突均返回错误并保留原木。
- Rust 状态转换测试覆盖 `Granted`、满包 `DroppedToGround`、结构错误、缺资源和 ID 冲突；每个失败断言联合终态为：原木未 harvested、方块非 AIR、库存/地面掉落无新增、无 `GatheringCompleteEvent`、terminal `completed=false`。

当前代码基线的本地 gate 与同 SHA GitHub gate：server `fmt + clippy + cargo test` 全绿（11,708/0/1）；Python protocol 96/96；Rust freshness bit pin 1/1；GitHub e2e run `29457375033`（job `87497219493`）全绿。归档文档若由后续 doc-only commit 更新，不可能在自身内容内自指其最终 commit；因此以此代码基线作为受验代码，并在 PR Body 同步当前 head、run 和 `git diff --check`/祖先关系。

## Finish Evidence

### 落地清单

- P0：在 `server/src/spiritwood/mod.rs` 增加生产链集成测试，修复前明确复现满包无地面掉落、原木却被错误标记 harvested 并置 AIR。
- P1：`grant_ling_mu_gun` 复用 `add_item_to_player_inventory_or_ground`；同一 customization 闭包覆盖入包与落地，保留 `spirit_quality`、freshness、数量、原木中心位置和 session 维度；freshness registry/profile 缺失 fail closed。
- P2：`complete_spiritwood_sessions` 通过 `grant_before_removing_session` 保证 grant 返回后才移除 completed session；仅在 `Granted` / `DroppedToGround` 后提交 harvested、AIR 和完成事件；结构错误、缺掉落 registry、缺 inventory、instance id 冲突均保留原木并发送 `completed=false`。
- P3：`proto/bong/envelope.proto` 与 Rust/Python converter 镜像 freshness 六字段；Bot 支持真实 `C2S_PLAYER_ACTION` 和 `lumber_progress`；生产五 tile fixture 固定 seed `(1292, 73, 1519)`、外缘主干 `(1285, 73, 1509)`，并以正式 `0..4` biome palette 约束所有 tile；专项场景验证满包落地与拾回。代码基线 `2cb2cfd078af744b40af4c4690499f0bee99c7e1` 的同 SHA PR gate run `29457375033`（job `87497219493`）全绿；旧 run `29422011676` 仅作历史证据。
- P3 运行隔离：`scripts/bot-e2e.sh` 为自启 server 分配独立 `BONG_SPIRITWOOD_HARVESTED_PATH`，防止上一轮已采伐持久化污染重跑。

### 关键 commit

- `41a45224`（2026-07-15）：升格 active plan，收口灵木满包原子结算范围。
- `2a3fa760`（2026-07-15）：加入修复前失败测试，证真满包吞产物与错误世界提交。
- `edc5ed34`（2026-07-15）：接入 inventory-or-ground grant，重排成功提交顺序并补齐边界测试。
- `ed6b2773`（2026-07-15）：合并最新 `origin/main`（PR #1210）并在合并结果上复验完整 server 门禁。
- `db58e582`（2026-07-15）：补齐 `InventoryItemView` freshness protobuf 与 Rust wire 镜像。
- `e8d65f99`（2026-07-15）：让 freshness registry/profile 缺失 fail closed，并锁定不消耗原木。
- `5a874df9`（2026-07-15）：新增生产五 tile fixture、真实 digging Bot 场景及协议饱和测试。
- `ce08d8e5`（2026-07-15）：普通 merge `origin/main@6f1faea5`，保留双方 Bot 能力并复验灵木生产链。
- `fc546e2a`（2026-07-15）：修复五 tile fixture 的 biome palette 映射，并增加逐 tile 文件长度与索引边界测试。
- `6508e685`（2026-07-15）：锁定真实采伐 `created_at_tick > 0`，并收紧 digging sequence 的 32 位 VarInt 边界。
- `d89af9da`（2026-07-15）：修复 grant 前提前移除 completed session 的原子顺序，并锁定成功与失败 grant 时 session 仍在 store。

### 测试结果

- 修复前新增契约测试：18 pass / 7 fail；失败明确显示 `DroppedLootRegistry=0`，同时原木已 harvested。
- `TMPDIR="$PWD/target/tmp" cargo test spiritwood::tests`：25 passed / 0 failed。
- `TMPDIR="$PWD/target/tmp" cargo test spiritwood::`：44 passed / 0 failed。
- `d89af9da` 定向顺序契约：成功与失败 grant 两例 2 passed / 0 failed；`cargo test spiritwood::` 50 passed / 0 failed。
- `freshness.initial_qi` 精度契约：Rust `inventory_item_view_freshness_survives_proto_roundtrip_for_all_tracks` 1 passed / 0 failed，并对拍 `f32::to_bits()`；Python protocol 全量 96 passed / 0 failed，并对拍相同 `0x42C50001` wire bits。
- `d89af9da` 执行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：lib 11,708 passed / 0 failed / 1 ignored，CLI 11 passed，full-app startup 1 passed，背包 e2e 4 passed，doc tests 0 failed。
- 历史 Rust 基线 `ce08d8e5` 执行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：lib 11,706 passed / 0 failed / 1 ignored，CLI 11 passed，full-app startup 1 passed，背包 e2e 4 passed，doc tests 0 failed；最终 Rust 代码门禁以 `d89af9da` 的同 SHA 结果为准。
- PR HEAD `e73ebc35` 的 GitHub e2e run `29413908258` 全绿（23m51s）：client、schema、agent、server 全测、Smoke/E2E、Bot e2e 与 artifact upload 均成功（历史证据）。
- 代码基线 `2cb2cfd078af744b40af4c4690499f0bee99c7e1` 的 GitHub e2e run `29457375033` 全绿：`e2e` job success，head SHA 与 PR head 对拍一致；job `87497219493` 的 client、schema、agent、server、Smoke/E2E、Bot e2e 和 artifact upload 均成功。
- 最新协议代码 HEAD `6508e685`：Python protocol 96 passed / 0 failed；`InventoryItemView` freshness 覆盖存在/缺省、六字段保真与三种 track，`lumber_progress`、digging packet 32 位边界与五 tile biome palette 边界均有 pin 测试。
- `6508e685` 完整 Bot e2e：第二轮 29 passed / 0 failed；生产灵木专项 13.5s，并以 `created_at_tick > 0` 锁定 tag 1 真实过线。首轮专项同样通过，但无关 `combat_weapon_equip_damage` 单次命中观察失败；第二轮该场景 21.9s 通过。
- 协议代码基线 `85360d2f` 之后的 `2cb2cfd0` 仅更新本归档证据文档，不改变已全门禁通过的 Rust/协议代码树；代码基线 `2cb2cfd0` 与同 SHA GitHub e2e run `29457375033` 的 head 对拍记录见上文，旧 run 仅作历史证据。
- client 使用 Temurin JDK 17.0.19 执行 `./gradlew test build`：BUILD SUCCESSFUL，13 tasks。
- 主线同步前后 `production_spiritwood_full_inventory_drop.py` 均通过；同步后首次完整 Bot e2e 的 `combat_skill_cast` 因 40 格观察时序抖动单次失败，定向连续两次通过（0.8s / 0.7s），随后完整 29/29 通过。
- `6508e685` 执行 `git diff --check origin/main...HEAD` 通过；工作树干净，merge-base 为 `origin/main@6f1faea5`，因此主线是最新协议代码 HEAD 的祖先。

### 跨仓库核验

- server：`complete_spiritwood_sessions`、`grant_ling_mu_gun`、`GrantOrGroundOutcome`、`DroppedLootRegistry`、`inventory_item_view_to_proto` 与五 tile fixture 均命中；`Freshness.initial_qi: f32` 与 protobuf `float` 逐 bit 对拍；fixture 的原始 `biome_id=4` 对应 manifest `minecraft:meadow`，不存在 palette 越界或 fallback。
- Bot / protocol：`Bot.start_digging` 发真实 `C2S_PLAYER_ACTION`；`proto_min.py` 解码 `lumber_progress`、掉落 snapshot 和 freshness 六字段，并以 `<f` 对拍 server 的 f32 bit pattern；专项场景实际清包并拾回同一实例。
- client：协议消费面未新增 UI 逻辑；JDK 17 完整 build 通过。agent/schema、Redis key 与正式 worldgen 资源格式不变。

### 遗留 / 后续

- `ChunkLayer` 暂不可写时仍沿用既有“记录 harvested、稍后由世界生成态收口”的语义，不在本 BugFix 扩成跨资源事务。
- 用户明确要求主 agent 直接实施且不启动 subagent，因此没有独立 subagent validator；本文不记录或伪造独立 validator PASS，最终 HEAD 由主 agent 只读复核。PR 合并前仍以最新 SHA 的 `/review`、CodeRabbit 和 e2e 为最终 gate。
