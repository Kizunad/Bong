# plan-forge-session-entry-wiring-v1 — 武器锻造起炉入口接线：打通 ForgeStartSession/ForgeBlueprintTurnPage 双端断链

> **一句话**：锻造 session 引擎、四步状态机、UI 面板、schema/proto 全部已实装，但**起炉入口两个 C2S 变体的分发被丢弃**（server handler 打 debug log 吞掉、client 无 sender）——整个武器炼器玩法对真实客户端不可达，只能测试驱动。本 plan 补最后一层接线 + 全链路 bot e2e。
>
> 来源：2026-07-07 三产三用 bot 测试（PR #1072）实测「协议死路」，用户拍板立 plan 收口。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 分发接线（ForgeStartSession → StartForgeRequest / TurnPage → 图谱书） | ✅ 2026-07-08 |
| P1 | client sender（ForgeScreen 起炉按钮 + 图谱书翻页发包） | ✅ 2026-07-08 |
| P2 | 放砧/起炉/结算回执补齐 + 全链路 bot e2e（fan_tie×3 → iron_sword 入包） | ✅ 2026-07-08 |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**（全部已实装，本 plan 只做接线，引用两份归档 plan 为源）：
  - `docs/finished_plans/plan-forge-v1.md`（引擎 MVP）+ `plan-forge-leftovers-v1.md`（UI 面板/中途三步接线）
  - server 引擎：`forge/session.rs`（`ForgeSession` 四步状态机 Billet/Tempering/Inscription/Consecration + `ForgeSessions` 总表）、`forge/mod.rs::handle_start_forge_requests`（:141，**现为空读**——`StartForgeRequest` 全仓零 send）、`forge/steps.rs`（纯函数+单测齐）、`forge/blueprint.rs`（`assets/forge/blueprints/*.json`，最简 `iron_sword_v0` = `fan_tie`×3）
  - schema/proto：`ClientRequestV1::ForgeStartSession{v,station_id,blueprint_id,materials}`（client_request.rs:605）/`ForgeBlueprintTurnPage`（:636）、proto 桥 proto_convert.rs:3934/3981——**wire 层就绪**
  - client：`ForgeScreen.java` + Tempering/Inscription/Consecration 面板组件 + 4 个 store + 4 个 handler——**面板就绪、无 sender**
- **出料**：起炉后走既有 `ForgeSessions` → 中途三步（已接线的 `forge_tempering_hit`/`forge_inscription_scroll`/`forge_consecration_inject`/`forge_step_advance`）→ `inventory_bridge` 结算武器入包 → `ForgeOutcomeEvent`
- **共享类型**：复用 `StartForgeRequest`（forge/events.rs:11，已定义未消费——接线即救活）；**不新造任何 event/schema**
- **跨仓库契约**：server `handle_client_request` 两分支分发；client `ForgeScreen`/blueprint book 发 `forge_start_session`/`forge_blueprint_turn_page`；agent 不参与
- **worldview 锚点**：炼器归 worldview 锻造/法宝章（plan-forge-v1 已锚定，本 plan 纯接线不动正典）
- **qi_physics 锚点**：开光注真元已由既有 `forge_consecration_inject` 链路走守恒（bughunt qc-P1 #580 修过通胀），本 plan 不新增真元路径

## §2 断链证据（2026-07-07 实测 + Explore 库存）

- `client_request_handler.rs:2601-2606`：`ForgeStartSession | ForgeBlueprintTurnPage => tracing::debug!("plan-forge-v1 client_request not yet wired")` —— 请求被静默丢弃
- `StartForgeRequest` 的 `.send()` 全仓 **0 处**（仅测试 `insert_test_forge_session` 塞会话）→ `handle_start_forge_requests` 永远空读
- client 全仓 grep `ForgeStartSession`/`BlueprintTurnPage` **0 命中**——UI 面板存在但没有发包入口
- 两份归档 plan 的「遗留/后续」都**没登记**这个缺口（归档遗漏）；bot 场景 `production_forge_station_real_place` 已用宽容断言钉住现状
- 另两处观察面弱点一并收：放砧成功无专属回执（只有 inventory 扣除）；forge 结算 `forge_outcome` payload 需确认起炉打通后真实可达

## §3 阶段划分

- **P0 server 分发**：`:2601` 分支改为解析 `station_id/blueprint_id/materials` → 校验（station 存在/blueprint 已学或可用/材料足）→ send `StartForgeRequest`；`ForgeBlueprintTurnPage` 分发到图谱书翻页状态。拒因走 chat/forge payload（材料不足/砧不存在/blueprint 未学，对齐炼丹 `send_alchemy_error` 模式）。单测：受理/三类拒因/materials 形状（`Vec<(String,u32)>`）
- **P1 client sender**：`ForgeScreen` 起炉按钮接 `ClientRequestSender`（对齐 dying_elder G 键给丹模式）；blueprint book 翻页发包。gradle test + 面板可视验证
- **P2 e2e + 回执**：升级 `scripts/bot/scenarios/production_forge_station_real_place.py` → 全链路（放砧→learn_blueprint→start_session→tempering_hit×N→step_advance→outcome→iron_sword 入包）；放砧回执补 `forge_station` payload 推送（对齐 alchemy open_furnace 模式）。**bot 场景是本 plan 验收门**（AGENTS.md §15）

## §4 开放问题（转 active 前收口）

1. **起炉是否需要先 learn blueprint**：`forge_learn_blueprint` 已接线；起炉校验是否强制已学（推荐强制，图谱书是玩法核心循环）还是 tier0 蓝图免学（新手 iron_sword_v0 白名单）。
2. **TurnPage 的语义落点**：图谱书是纯 client 状态（翻页无需 server）还是 server 权威页码（防作弊翻到未学页）。推荐 server 权威（已有 `forge_blueprint_book` payload 通道）。
3. **station_id 契约**：wire 字段是 `station_id: u64`，但砧实体用 BlockPos 定位（alchemy 用 furnace_pos）——接线时统一成哪种寻址（推荐对齐 alchemy 的 pos 寻址，wire 已定型则 station_id→pos 映射表）。

> 全部已在 §4.1 收口。原表保留以备追溯，**实施时以 §4.1 决议为准**。

## §4.1 决议（pre-P0 收口，2026-07-08）

> 决议数据来自转 active 前 Explore agent 全接口面实测（只读核查 server/client/schema/bot 四面），非拍脑袋。

### #1 起炉强制已学蓝图

**决议**：
1. 强制已学——起炉校验 `LearnedBlueprints::knows(blueprint_id)`，未学拒绝并回执。
2. **引擎已经是这个语义**（`handle_start_forge_requests` 内已强制校验），接线层不改引擎、不加白名单。
3. 拒绝 tier0 免学路线：白名单绕过图谱书核心循环，且需改引擎语义，改动面反而更大。e2e 场景先走已接线的 `forge_learn_blueprint`（give 残卷 → learn）再起炉。

**落点**：`server/src/forge/mod.rs:158-163`（`learned.get(req.caster)` + `lb.knows(&bp.id)`，现状即决议）/ `server/src/network/client_request_handler.rs:3127-3207`（`handle_forge_learn_blueprint` 已接线，e2e 前置步骤）/ plan §3 P2（e2e 步骤含 learn_blueprint）。

### #2 TurnPage server 权威页码

**决议**：
1. server 权威——`ForgeBlueprintTurnPage` 分发到 `LearnedBlueprints::next_page/prev_page`，翻页后回推 `forge_blueprint_book` S2C。
2. client 翻页键改为发 `forge_blueprint_turn_page` C2S，页码状态由 `ForgeBlueprintBookHandler` 从 S2C 更新；**删除 `BlueprintScrollStore.turn` 的本地直改路径**（不做本地乐观+校正双路径）。
3. 拒绝纯 client 状态路线：起炉请求需要 server 知道"当前选中蓝图"才能防作弊，且 `forge_blueprint_book` payload 通道已存在，成本为零。

**落点**：`server/src/forge/learned.rs:45-58`（`next_page`/`prev_page` 已实现）/ `server/src/network/forge_snapshot_emit.rs:201-214`（`build_blueprint_book`）/ `client/src/main/java/com/bong/client/forge/state/BlueprintScrollStore.java:29-32`（本地 `turn` 改发包）/ `client/src/main/java/com/bong/client/network/ServerDataRouter.java:203-214`（handler 已注册）/ plan §3 P0+P1。

### #3 寻址统一 pos，wire 直接改形状

**决议**：
1. `ForgeStartSession` 的 `station_id: String`（骨架误记 u64，实为 String）**改为 `station_pos: (i32,i32,i32)`**，全栈对齐 alchemy 的 `furnace_pos` 寻址模式；server 按 pos 查 `Query<&WeaponForgeStation>` 解析出 `StartForgeRequest.station: Entity`。
2. **不做 station_id→pos 映射兼容层**：该 wire 字段从未有真实发送方（client 零 sender、server 分支丢弃），改形状零迁移成本；这是改对形状的最后窗口。
3. 拒绝 String id 路线：唯一现成反向 id 是 `format!("forge_station_{}", entity.to_bits())`，entity bits 跨重启不稳定且暴露内部句柄；无 StationRegistry，凭空造注册表违反"复用已有寻址模式"。
4. 同步面（schema 改动连同 sample 一起改）：TypeBox + sample + generated schema、proto message、proto_convert + fixtures、serde struct、bot 场景（现发 `station_id: 1` 整数，本就与 String schema 不符，一并修正）。

**落点**：`agent/packages/schema/src/client-request.ts:940-951` / `agent/packages/schema/samples/client-request.forge-start.sample.json` / `proto/bong/envelope.proto:1028-1033`（`ForgeStartSession`）/ `server/src/schema/client_request.rs:605-610` / `server/src/schema/proto_convert.rs:3937-3952` + fixtures `:7752-7757` / 寻址助手对齐 `client_request_handler.rs:12603-12640`（`with_owned_furnace_mut` 模式，owner 校验一并对齐）/ `server/src/forge/station.rs:26-33`（`block_pos()`）/ `scripts/bot/scenarios/production_forge_station_real_place.py:75-85` / plan §3 P0。

### #4（新发现缺口，收进 P0 范围）起炉必须原子扣输入料

**决议**：
1. 现状：`handle_start_forge_requests` 只写 `session.committed_materials`（记账），背包分文不扣；`inventory_bridge` 只发放产物——起炉是凭空造物，e2e「fan_tie×3 → iron_sword 入包」会把这个缺口锁成合法行为，必须先补。
2. 扩 P0 交付物：全部校验（blueprint 存在/已学/砧 tier/材料/billet）通过后、会话建立的同一受理点原子扣除输入料；**任何拒绝路径（含 Waste billet）不得吞料**。扣料走既有 `consume_item_instance_once` 族 + `resync_snapshot`（对齐放砧扣料模式）。
3. P2 e2e 终态断言同步扩：产物入包 +1 且输入料从背包消失；拒因路径（材料不足）断言背包原封不动。
4. 材料是物品非真元，不涉 qi_physics ledger；开光注真元仍走既有 `forge_consecration_inject` 守恒链路，本 plan 不新增真元路径（与 §1 一致）。

**落点**：`server/src/forge/mod.rs:252-268`（建会话点，`committed_materials` 只记账）/ `server/src/forge/inventory_bridge.rs:73-141`（只发产物的现状证据）/ 扣料范式 `server/src/forge/station.rs:151-157`（放砧 `consume_item_instance_once`）/ plan §3 P0+P2。

## Finish Evidence

消费方式：`/consume-plan` Workflow 编排（Design opus → P0/P1/P2 串行 sonnet 实施 → 3 lens 对峙 + opus 主审 → 2 轮修复 + opus 复审），2026-07-08。

### 落地清单

- **P0 server 分发 + schema 改形状 + 原子扣料**：
  - `server/src/schema/client_request.rs`（ForgeStartSession `station_id:String`→`station_pos:(i32,i32,i32)` + 两变体 serde pin）
  - `server/src/schema/forge.rs` + `proto/bong/envelope.proto` + `server/src/schema/proto_convert.rs`（C2S/S2C 双向 + fixtures；WeaponForgeStationDataV1/proto ForgeStation 补 pos——U 键全局开屏的 client 寻址来源）
  - `agent/packages/schema/src/client-request.ts` + `src/forge.ts` + `samples/client-request.forge-start.sample.json` + `samples/server-data.forge-station.sample.json` + generated 全套
  - `server/src/network/client_request_handler.rs`（:2601 死分支→真分发：pos→Entity 解析含 owner 校验对齐 `with_owned_furnace_mut` 模式、`send_forge_error` 拒因回执；`handle_forge_blueprint_turn_page` server 权威页码 + `%len` 守卫）
  - `server/src/inventory/mod.rs`（`consume_forge_materials_atomic` + `ForgeMaterialDeficit`：verify-then-consume，`mineral_id||template_id` 匹配跨 container+hotbar 累加）
  - `server/src/forge/mod.rs`（`handle_start_forge_requests` 受理点原子扣料 + 未学/缺料/砧忙拒因回执 + `station.session.is_some()` 忙守卫）
  - `server/src/mineral/events.rs`（`forge_blueprint_not_learned` / `forge_materials_insufficient`（display_name_zh）/ `forge_station_busy` 构造器）
- **P1 client sender**：
  - `client/.../network/ClientRequestProtocol.java` + `ClientRequestSender.java`（`sendForgeStartSession(stationPos,blueprintId,materials)` / `sendForgeBlueprintTurnPage(delta)`）
  - `client/.../forge/ForgeScreen.java`（起炉入口 + billet 点选聚合；←/→ 翻页改发包，删 `BlueprintScrollStore.turn` 本地直改）
  - `client/.../forge/state/ForgeStationStore.java` + `network/forge/ForgeStationHandler.java`（station pos 存取）
- **P2 回执 + bot e2e**：
  - `server/src/forge/station.rs` + `network/forge_snapshot_emit.rs`（放砧推 forge_station；起炉受理/单步交互/step 推进接 `send_forge_snapshots_to_player`；结算接 `send_forge_outcome_to_player`——三个此前零调用的 emit 函数全部接活）
  - `server/src/cmd/`（/give 裸矿物 mineral_id fallback，供 e2e 上料）
  - `scripts/bot/proto_min.py`（forge S2C tag 17-20 解码）+ `scripts/bot/scenarios/production_forge_station_real_place.py`（全链路强断言：放砧回执→give 残卷→learn→拒因（缺料，背包原封）→受理→淬炼×10→结算→产物入包+输入料扣光；真机 fresh server 跑绿）
- **对峙修复轮**（1 blocker + 2 major 全修）：
  - `client/.../inventory/model/InventoryItem.java`（`mineralId` 字段 + `forgeMaterialKey()`——真机恒 Waste blocker：投料 key 曾发 template_id 而引擎只认 canonical）+ `InventorySnapshotHandler`/`InventoryEventHandler` 解析与复制路径保留
  - turn_page `%len` 守卫（单包 i32::MIN 冻结 tick DoS，对齐 alchemy 同款）
  - 砧忙守卫（双扣料 + 孤儿会话）+ 缺料回执中文化

### 关键 commit（squash 前 worktree 序列，2026-07-08）

f0e81dac schema pos 寻址 → e1eaeeca 原子扣料 helper → 445fabdb 拒因回执构造器 → 90de8bc0 引擎扣料接线 → ba108aeb C2S 真分发 → 5b6ccdbe client encode+sender → 7f5f4abb StationStore pos → b4a7977d ForgeScreen 起炉+翻页 → 3eaff7d1 S2C 回执全接线 → 9dd16056 /give 矿物 fallback → b983c5ba bot 解码 → 3797f540 bot 全链路场景 → 153ba6b4/fae95bda/32ee3e0a 对峙修复轮 → 4e811e15 复审第 2 轮（createFull 编译修复 + 文案）

### 测试结果

- `cargo test`（全量）：**10901 passed / 0 failed**（含 forge 系 284、turn_page 13、mineral::events 3）；`cargo fmt --check` 过；clippy 本地 69 错均为 rustc 1.96 存量噪声（全部位于本 plan 未触碰文件/行，以 CI 为准）
- `./gradlew test build`：**BUILD SUCCESSFUL**（真实退出码 0 验证；含 InventoryItem mineralId 5 pin + snapshot mineral_id 含/缺两路 + sender/protocol 契约测试）
- `agent packages/schema npm test`：schema/sample 对拍过（P0 实施内）
- bot：`test_protocol.py` 全绿 + `production_forge_station_real_place.py` 真机 fresh server 全链路绿（P2 实施内）

### 跨仓库核验

- server：`ClientRequestV1::ForgeStartSession{station_pos}` / `StartForgeRequest`（send :2601 分支）/ `consume_forge_materials_atomic` / `MSG_FORGE_STATION_BUSY`
- client：`sendForgeStartSession` / `forgeMaterialKey` / `ForgeBlueprintBookHandler`（页码 S2C 权威）
- agent schema：`ForgeStartSessionRequestV1.station_pos` + forge-start sample
- bot：`proto_min.py` tag 17/18/19/20 + 全链路场景

### 遗留 / 后续

1. **forge 会话无 abort/收回路径**（复审 medium）：玩家起炉后弃疗/掉线，砧被占住直到会话被推进到终态——忙守卫生效后无法靠"新起炉覆盖"救回。需 follow-up 立 ForgeCancel/断线清理语义（回执文案已改为不承诺"收回"）。
2. **`emit_join_forge_snapshots` 仍空占位**：join-hydration（上线时已在砧边应即收快照）不在本 plan 接线范围，占位系统仍每 tick 空跑。
3. proto `ForgeStartSession` 字段改号 nit：blueprint_id/materials 被无谓移位（2→4/3→5）——该 message 从无真实发送方，无迁移影响，纯纪律记录。
4. client 投料 key 派生路径（ForgeScreen 聚合）仅单测覆盖：bot e2e 在协议层直发 canonical，不经 client UI 聚合。
