# BugHunt: 延寿棺回收/破坏无所有权校验，任意玩家可窃取他人棺材料

## Bug 摘要

**严重度：high（skeptic 未调整，维持 high）**

`CoffinEntity`（`server/src/coffin/mod.rs:111-122`）从落地起就没有任何 owner / placed_by 字段，`handle_coffin_breaks`（破坏，L711-826）与 `handle_coffin_menu_reclaim`（G 菜单回收，L837-963）对请求者的唯一门控是 `coffin_target_is_close` 纯欧氏距离校验（L1217-1224），从未比对"谁放置了这口棺"。玩家 A 花费真实合成材料（`consume_item_instance_once`，L497）造棺放置后，任何靠近的玩家 B 只需正常游玩（左键攻击棺 marker 触发破坏、或打开 G 菜单点【回收】）即可把返还材料发进**自己**的背包（`grant_reclaim_drops_to_inventory` 恒对 `players.get_mut(event.player)` 即请求发起者本人生效，L795/L932），A 分文未得；若 A 或其他人正卧棺休眠，还会被强制弹出且无任何提示。这是标准联机场景下即可触发的偷窃/惊扰链路，不需要任何客户端改造或 dev 命令。

## 实际游玩体验影响

玩家 A 花骨币/材料合成 `jade_coffin` / `stone_coffin` / `bronze_coffin` 等高档棺、放置在自己的营地里准备用来续命/卧棺修炼；任何路过或蓄意跟踪的玩家 B，只需靠近到 6 格内左键攻击棺 marker（破坏，随机部分返还）或直接打开 G 菜单点【回收】（较全返还），就能把 A 造棺的合成材料全部搬进自己背包——A 全程不会收到任何通知，直到回来发现棺不见了。更恶劣的是：如果 A 正在棺里休眠（`in_coffin=true`），B 的破坏/回收会直接把 A 强制弹出卧棺状态、清除隐身，A 在毫无预警下被"掀棺"。这是典型的多人联机资产盗窃 + 强制骚扰漏洞，且成本极低（不需要任何特殊道具或权限，普通近战交互即可）。

## 证据定位

- `server/src/coffin/mod.rs:111-122`：`CoffinEntity { lower, upper, occupied_by, placed_at_tick, grade, marker_entity }` —— 全字段扫描确认没有 owner / placed_by / owner_player_id，全仓 `coffin/mod.rs` 内唯一命中 "owner" 的是无关的 `owner_instance_id: None`（物品实例占位字段，与棺放置者身份无关）。
- `server/src/coffin/mod.rs:131-147`（`CoffinRegistry::insert`）：签名只接受 `lower/placed_at_tick/grade`，没有 owner 参数，写入的 `CoffinEntity` 天然无所有者。
- `server/src/coffin/mod.rs:418-567`（`handle_coffin_place_requests`）：L439 已经从 ECS query 里拿到放置者的 `Username`，L497 `consume_item_instance_once` 真实扣掉玩家背包里的棺材实物（有真实成本），但 L504 `registry.insert(event.pos, event.tick, grade)` 从未把 `username` 写进 registry —— 放置成本是实打实的，所有权却从一开始就没被记录。
- `server/src/coffin/mod.rs:711-826`（`handle_coffin_breaks`）：L742-751 唯一门控 `coffin_target_is_close(position, event.pos)`；L783-818 把 `compute_coffin_reclaim_drops` 算出的返还材料通过 `grant_reclaim_drops_to_inventory` 直接发给 `players.get_mut(event.player)`（即破坏请求发起者，不是原放置者）。
- `server/src/coffin/mod.rs:837-963`（`handle_coffin_menu_reclaim`）：L874-883 同样只做距离校验；L920-955 同样把"较全返还"材料发给请求发起者 `event.player`。
- `server/src/coffin/mod.rs:1217-1224`（`coffin_target_is_close`）：纯 `distance_squared <= COFFIN_INTERACT_MAX_DISTANCE_SQ`（36.0）欧氏距离判断，与身份无关。
- Client 侧可达性确认（无需改客户端、无需 dev 命令即可触发）：
  - `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java:43-56`：`attackEntity` 注入对任意 `BongModeledEntity` 棺 marker 都会 `ClientRequestSender.sendCoffinBreak(target.getBlockPos())`，没有任何所有权过滤。
  - `client/src/main/java/com/bong/client/coffin/CoffinMenuScreen.java:53-54, 85-86`：【回收】按钮点击直接 `ClientRequestSender.sendCoffinMenuReclaim(coffinPos)`，同样无所有权过滤；该菜单由 `CoffinEnterIntentHandler` 右键/G 键在任意棺 marker 上打开。
  - `server/src/network/client_request_handler.rs`（`coffin_break` / `coffin_menu_reclaim` 分支）把上述 C2S payload 原样转成 `CoffinBreakRequest` / `CoffinMenuReclaimRequest` 事件，中间无额外校验层。
- 持久化侧核实（用于「风险」节的迁移策略判断）：
  - `CoffinRegistry` 本身是纯运行态 Bevy `Resource`（`server/src/coffin/mod.rs:311` `app.insert_resource(CoffinRegistry::default())`，`server/src/main.rs:100` `coffin::register(&mut app)`），未发现任何 SQLite/JSON 加载路径——服务器每次完整重启都会把 `CoffinRegistry` 清空重建，**不存在"数据库里躺着一批无 owner 的棺记录需要迁移"**的问题。
  - 但 `server/src/player/mod.rs:270-285`（`attach_player_state_to_joined_clients`）在玩家重连且 `persisted.in_coffin == true` 时，会调用 `registry.reclaim_occupied(coffin_lower, entity, 0, grade)`（`reclaim_occupied` 定义于 `server/src/coffin/mod.rs:199-221`）在 registry 里**凭空重建**一条 `CoffinEntity`——而 `player_lifespan` 表（`server/src/persistence/mod.rs` v27 迁移，约 L1978-1990）只持久化该玩家自己的 `in_coffin`/`coffin_grade`，从未记录"这口棺是谁放的"。这意味着即使加了 owner 字段，**每次玩家重连触发的 registry 重建路径也拿不到真实 owner 数据**，需要一个明确的默认策略（见「风险」节）。

## 触发路径

1. 玩家 A 合成任意档级延寿棺（`mundane_coffin`/`jade_coffin`/`stone_coffin`/`bronze_coffin`）并通过 `CoffinPlace` C2S 正常放置——server 真实扣除 A 背包里的棺材物品，registry 写入一条无所有者信息的 `CoffinEntity`。
2. 任意玩家 B（无需修改客户端、无需 dev 命令）靠近该棺 6 格内。
3. B 左键攻击棺 marker 实体 → 客户端 `MixinClientPlayerInteractionManagerAlchemy` 发送 `coffin_break` C2S；或 B 右键/G 键打开 `CoffinMenuScreen` 点【回收】→ 发送 `coffin_menu_reclaim` C2S。
4. server `handle_coffin_breaks` / `handle_coffin_menu_reclaim` 只校验 B 是否离棺够近，从未比对 B 是否为放置者 A；随后把返还材料发进 B 的背包，从 registry 移除该棺，若棺内有人（无论是 A 还是任何借宿者）强制弹出。
5. A 事后发现棺不见了、材料没有回到自己背包；若 A 当时正在棺内休眠，还会在毫无预警下被强制弹出隐身状态。

## 反方审查记录

- 第一轮质疑：
  - 怀疑"也许 `coffin_target_is_close` 之外还有别的隐式所有权门控"——复核 `handle_coffin_breaks`/`handle_coffin_menu_reclaim` 全函数体，确认门控只有距离校验一层，无 owner 比对。
  - 怀疑"也许 `CoffinEntity` 在别处（如 marker 实体的自定义 component）挂了 owner"——grep 全仓 `coffin/mod.rs` 的 "owner" 命中，唯一结果是与放置者身份无关的 `owner_instance_id: None`（物品实例占位字段）。
  - 怀疑"客户端 UI 是否本身就限制了只有放置者能看到回收按钮"——复核 `CoffinMenuScreen`/`CoffinEnterIntentHandler`，菜单对任意玩家在任意棺 marker 上都可打开，无所有权过滤；且 server 授权本就不能依赖客户端可见性。
  - 初裁：倾向真 bug，进入第二轮补证。
- 第二轮补证：
  - 核实与已在跑的 `docs/plan-bughunt-coffin-dimension-gate-v1.md`（覆盖同一批 handler 但仅补维度门禁，"修复方向"通篇未涉及所有权比对）不冲突——两者是同一批函数上的两个正交缺口，互不覆盖，互不阻塞。
  - 核实与 `docs/plans-skeleton/plan-coffin-offline-reclaim-respawn-dup-v1.md`（覆盖"离线卧棺占用态在断连/重连之间自相矛盾，导致抢棺/幽灵棺"）根因不同——该骨架的核心矛盾是 `occupied_by` 在断连时被清空又在重连时被 `reclaim_occupied` 凭空补回，与"棺从落地起就没有 owner 概念"是两回事：即便修好占用态的断连/重连状态机，只要 `CoffinEntity` 没有 owner 字段，任何在线玩家 B 依然可以在 A 从未断线的情况下直接靠近偷棺，二者必须分别修复。
  - 补充核实：`docs/finished_plans/plan-coffin-v1.md`、`plan-coffin-tiers-v1.md` 均未见任何"允许任意玩家回收他人棺"的设计意图——Break/Reclaim 的设计文档只按档级（grade）和模式（Break vs Reclaim）区分返还比例，从未讨论过权限主体，说明当前行为是遗漏而非有意的社交/共享设计。
  - 补充核实：`handle_coffin_enter_requests`（L569-639）本身也不做所有权校验（任何人可躺进任意未被占用的棺）——但这是"共享卧具可被任何人使用"的合理留白（不产生材料收益、也不消耗放置者资产），与 Break/Reclaim 这种"直接把放置者已花费的合成材料转移给他人背包"的经济漏洞性质不同，故本 plan 的修复范围严格限定 Break/Reclaim，不动 Enter。
  - 终裁：通过，high 严重度维持不变。
- 主循环复核：已亲读关键行确认（`CoffinEntity` 结构体 L111-122、`CoffinRegistry::insert` L131-147、`handle_coffin_place_requests` L418-567、`handle_coffin_breaks` L711-826、`handle_coffin_menu_reclaim` L837-963、`coffin_target_is_close` L1217-1224、`player/mod.rs` 重连重建路径 L270-285、client 侧 `MixinClientPlayerInteractionManagerAlchemy.java`/`CoffinMenuScreen.java` 均在本 worktree `b398c4071` 实地读码验证，行号与 JSON 原始引用基本一致，无需改动）。

## Skeleton Fix Plan

- [ ] 给 `CoffinEntity`（`server/src/coffin/mod.rs:111-122`）新增字段 `owner_player_id: String`，采用与 `cultivation`/`life_record` 一致的 `crate::player::state::canonical_player_id(username)` 规范化（不直接存原始 `Username`，避免大小写/改名漂移）。
- [ ] `CoffinRegistry::insert`（L131-147）签名扩展为接收 `owner_player_id: String` 参数，写入 `CoffinEntity` 时带上；`reclaim_occupied`（L199-221）在 `unwrap_or(CoffinEntity { ... })` 回退分支同样需要显式 owner（见下一条重连策略）。
- [ ] `handle_coffin_place_requests`（L418-567）在 L497 消费物品成功、L504 调 `registry.insert` 时，把 L439 已取到的 `username.0.as_str()` 走 `canonical_player_id` 传入，写入真实放置者身份——**此调用点已经查询了 `Username`，不需要新增 query 字段，纯本地改动**。
- [ ] `handle_coffin_menu_reclaim`（L837-963）在 L866 `registry.lookup` 拿到 `coffin` 后、真正执行 `remove_by_pos`/返还材料之前，插入所有权校验：取 `event.player` 的 `Username` → `canonical_player_id` → 与 `coffin.owner_player_id` 比较；不一致则 `tracing::warn!` + 直接 `continue`（不 `remove_by_pos`、不 despawn marker、不 grant 材料、不弹出占用者），并给请求者发一条聊天拒绝反馈（可参照 `server/src/world/container_open.rs` 对占用会话的拒绝话术风格，构造"这不是你的棺"类提示）。**推荐默认方案**：Reclaim 是"我要回收自己的材料"的明确主动经济行为，必须严格 owner-only，无豁免。
- [ ] `handle_coffin_breaks`（L711-826）设计决策——**需要人工在两个选项间拍板，不由实施 subagent 自行决定**：
  - **选项 A（推荐默认）**：与 Reclaim 对齐，非 owner 一律拒绝整个破坏动作（不 despawn、不返还、不弹人），棺对非 owner 而言表现为"打不动"。优点：与 Reclaim 语义统一，杜绝任何非 owner 收益路径；缺点：owner 永久离线后，废弃棺会一直占地无法被其他玩家清理。
  - **选项 B（豁免）**：允许任何人破坏清场（`remove_by_pos`/despawn marker 正常执行，允许弹出占用者），但材料返还目标固定发给 `coffin.owner_player_id`（owner 不在线则丢弃返还，不发给破坏者），破坏者自己颗粒无收。优点：保留"清理废弃棺腾地"的世界维护性；缺点：破坏者对着别人棺白忙一场的体验需要清晰的拒绝/告知反馈，且"owner 不在线材料去哪"需要再定策略（发进邮箱式离线补偿 vs 直接丢弃都要守恒律友好，若走"丢弃"必须确认材料本身不含真元载荷，不触发灵气蒸发红旗）。
  - 若无人工介入，实施 subagent **默认落地选项 A**（更简单、无二义性、不引入新的离线材料路由问题），并在归档 plan 文档时明确写下"选了 A，B 留作后续 dev 命令/清理系统的备选"。
- [ ] `handle_coffin_menu_reclaim`/`handle_coffin_breaks` 校验必须放在任何 `remove_by_pos`/`consume`/`grant_reclaim_drops_to_inventory` 副作用之前——校验失败必须是纯粹的 no-op continue，不能出现"先扣棺后才发现不该扣"的部分副作用。
- [ ] **server gate 是最终权威，client 隐藏只是 UX 增强**：可选给客户端加"非棺主人不显示【回收】按钮/攻击棺 marker 无反馈"之类的提示优化，但这只是体验糖衣；即使客户端被绕过、伪造 C2S 或使用旧版本客户端，server 端的 owner 比对必须独立生效，不依赖客户端配合。
- [ ] `server/src/player/mod.rs:270-285`（重连时 `reclaim_occupied` 重建 registry 记录）：由于 `player_lifespan` 表从未持久化"棺是谁放的"，重建时无法查到真实 owner。**推荐默认策略**：把 owner 临时置为"当前重连的这个占用者自己"（`canonical_player_id(username)`），即认定"重连时被重建的这条记录，其占用者临时兼任 owner"。这是已知的启发式限制（见「风险」节），不阻塞本次修复落地，但必须在代码注释和 Finish Evidence 里显式记录这个已知边界情况。
- [ ] 为 `handle_coffin_place_requests` 补回归：owner 写入 registry 正确（放置后 `registry.lookup(pos).unwrap().owner_player_id == canonical_player_id(placer_username)`）。
- [ ] 为 `handle_coffin_menu_reclaim` 补新测试：非 owner 请求被拒绝——registry 记录不消失、marker 不 despawn、请求者背包无新增物品、`CoffinStateChanged`/`PlaySoundRecipeRequest` 不发出；owner 请求正常通过，行为与现状回归一致（较全返还、marker despawn、占用者弹出）。
- [ ] 为 `handle_coffin_breaks` 补新测试：按落地的选项 A/B 分别锁定对应行为（选 A：非 owner 请求整体 no-op；选 B：非 owner 请求执行清场但材料进 owner 背包，破坏者背包不变）。
- [ ] 为 `reclaim_occupied` 重连重建路径补一条 pin 测试：`persisted.in_coffin=true` 且 registry 里该 pos 尚无记录时，重建出的 `CoffinEntity.owner_player_id` 精确等于重连玩家自己的 canonical id（锁定本 plan 选定的启发式默认值，避免未来重构悄悄改成别的默认）。

## 验收测试计划

全部在 `server/` 用 `cargo test` 跑，复用文件内已有的 `app_with_break_system()` / `app_with_reclaim_system()` / `register_mundane_coffin_with_marker` 等测试 helper（`server/src/coffin/mod.rs` 现有测试段），多玩家场景复用 `server/src/world/container_open.rs` 里已验证过的 `spawn_test_player(&mut app, "Bramble", [x, y, z])` 二人模式：

- **happy path（owner 操作，回归不能坏）**：
  - `handle_coffin_place_requests`：玩家 A 放置棺后，`CoffinRegistry::lookup` 返回的 `CoffinEntity.owner_player_id == canonical_player_id("A")`；同时确认现有行为不回归（物品仍被 `consume_item_instance_once` 扣除、marker 正常 spawn）。
  - `handle_coffin_menu_reclaim`（owner=A 请求）：registry 移除、marker despawn、A 背包精确收到 Reclaim 全量材料——与现有 `ecs_coffin_menu_reclaim_despawns_marker_and_grants_full_materials` 行为一致，不得回归。
  - `handle_coffin_breaks`（owner=A 请求）：registry 移除、marker despawn、A 背包收到 Break 随机部分返还——与现有 `ecs_coffin_break_despawns_marker_and_grants_partial_materials` 行为一致，不得回归。
- **边界（owner 缺席/占用者非 owner）**：
  - A 放棺后离线，未占用状态下 B 靠近发起 Reclaim/Break：拒绝路径必须触发（下一条）。
  - A 造棺后允许陌生人 C 通过 `CoffinEnterRequest` 躺进去（Enter 本身不做所有权校验，属设计内），随后 B（非 A 非 C）发起 Break/Reclaim：校验对象必须是 `coffin.owner_player_id`（即 A），与当前占用者 C 无关；B 依旧被拒绝。
  - A 自己发起 Reclaim/Break 但棺当前被 C 占用（C 借宿）：owner 校验必须放行（A 是 owner），C 被强制弹出属预期行为，需断言 `CoffinComponent` 从 C 身上移除、C 的 `Position`/`Flags.invisible` 复位。
- **错误分支（非 owner 请求，核心新增行为）**：
  - B（非 owner）发起 `CoffinMenuReclaimRequest`：断言——① registry 该 pos 仍 `lookup` 得到原棺记录（未被 `remove_by_pos`）；② marker 实体仍存在于 world（`app.world().get_entity(marker).is_some()`）；③ B 的 `PlayerInventory` 无任何新增 item instance；④ 未发出 `CoffinStateChanged`/`coffin_menu_reclaimed` 相关的 `PlaySoundRecipeRequest`；⑤ 若棺当前有占用者，占用者未被弹出。
  - B（非 owner）发起 `CoffinBreakRequest`：按落地选项分别断言。若选 A（默认）：五项断言与上面 Reclaim 完全对称（纯 no-op）。若选 B（豁免清场）：断言 registry 记录被移除、marker despawn、但返还材料进的是 A（owner）的背包而非 B 的背包，且 B 背包无新增物品。
  - 空边界：owner 已从服务器彻底移除角色数据（如通过 `/clearinv` 或角色被终结）后，`canonical_player_id` 仍可正常计算字符串比较（不 panic），此时任何人（含曾经的占用者）发起 Reclaim/Break 都应遵循同一非 owner 拒绝规则（不因"owner 找不到"而自动放行）。
- **状态转换（重连重建路径）**：
  - 模拟 `persisted.in_coffin=true` 且 `CoffinRegistry` 中该 pos 无记录（模拟服务器重启后玩家重连）：走 `attach_player_state_to_joined_clients` → `reclaim_occupied` 路径，断言重建出的 `CoffinEntity.owner_player_id` 精确等于重连玩家自己的 canonical id（锁定本 plan 选定的默认启发式）。
  - 同一玩家重复触发 `reclaim_occupied`（A→A 状态转换，例如二次重连）：owner 字段保持不变，不应被覆盖成别的身份。
  - 若 `reclaim_occupied` 命中 registry 里已有记录（棺已存在，只是重新指定占用者，如 offline-reclaim-respawn-dup-v1 骨架描述的场景），断言 owner 字段不会被本次调用意外覆盖（只应更新 `occupied_by`，不应连带篡改 `owner_player_id`）——这条测试同时为将来消费 `plan-coffin-offline-reclaim-respawn-dup-v1` 提供交叉验证锚点，防止两个 fix 互相踩踏。

## 风险

- **CoffinRegistry 本身无持久化，不存在"数据库老棺无 owner 需批量迁移"的问题**——它是纯运行态 `Resource`，每次服务器完整重启都会清空重建；本次修复不需要写任何 SQLite migration。
- **但重连重建路径（`player/mod.rs:270-285` → `reclaim_occupied`）天生拿不到真实 owner**：`player_lifespan` 表只持久化"我自己是否在棺内"，从未记录"这口棺是谁放的"。本 plan 选定的默认策略——"重建时把占用者本人临时当作 owner"——在绝大多数场景成立（玩家通常睡在自己的棺里），但如果重启前正好是 B 借宿在 A 的棺里、A 未占用，重启后 B 重连会被错误地"扶正"成该棺的 owner，A 反而失去对自己造的棺的所有权。这是已知且经过权衡接受的边界情况，必须在 Finish Evidence 里显式记录，不允许后续静默"修好"却不声明这个 trade-off 已变化。
- **`handle_coffin_breaks` 的选项 A/B 二选一是本 plan 唯一需要人工确认的设计决策**：选 A（严格 owner-only，默认）会让废弃棺永久占地，只能靠 owner 自己回来清或未来的管理员/清理系统处理；选 B（允许清场但材料归 owner）保留了世界维护性，但引入"owner 不在线时返还材料去哪"的新问题，处理不慎（比如返还材料时凭空创建新的真元/丹药实例而不做等价扣减）本身可能违反资产守恒直觉——不过棺材料返还是普通 `ItemInstance`（合成材料，非真元/灵气），不触碰 `qi_physics::ledger`，所以选 B 若真去做，风险集中在"库存/物品对象是否被正确创建或丢弃"而非真元凭空产生，二者不要混为一谈。
- **修复点必须在任何副作用之前**：如果把 owner 校验放在 `remove_by_pos`/`grant_reclaim_drops_to_inventory` 之后，只会把"材料被偷"变成"棺被清空但材料两边都没拿到"的更糟状态；必须严格保证校验失败路径是纯粹 no-op。
- **不要顺手扩大范围到 `handle_coffin_enter_requests`**：Enter（进棺休眠）当前允许任何人使用任意未占用的棺，这是共享卧具的合理留白，不产生材料收益，本 plan 明确不触碰，避免把一个经济漏洞 bug 修复变成"棺材使用权"的设计改动。
