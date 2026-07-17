# plan-bughunt-ac-alchemy-hud-zero-targets-v1

> **状态：finished（2026-07-17 最终验收；2026-07-15 promotion / 归档）**。来源：
> `docs/plans-skeleton/plan-bughunt-ac-alchemy-hud-zero-targets-v1.md`（2026-07-05）。
> 一句话主题：真实炼丹链路里，server 发给 client 的 `alchemy_session` payload 把
> **`target_ticks` / `temp_target` / `temp_band` / `qi_target` / `stages`** 全部写成 0
> 或空数组，导致炼丹 HUD 与炉内 UI 在玩家真正起炉后失去核心引导信息。

> **这个 bug 对实际游玩体验的影响**：玩家按 `I` 起炉后，中央炼丹 HUD 会长期显示 **`炼制 0%`**，温度条颜色判断失真，炉内面板会显示 **`elapsed / 0t`**、`当前火候 / 0.00`、`已注真元 / 0.0`，而且中途投料槽位不会按窗口闪烁提示。结果是玩家只能盲炼，无法根据 UI 判断何时收丹、火候是否贴近目标、还差多少注气、哪一槽该在当前 tick 投料；这直接削弱 `plan-alchemy-v1` 设计里“火候+阶段窗”的可玩性。

## P0 — `alchemy_session` 活跃链路零目标值（fix_pr）

- **高置信 bug（fix_pr）**：`server/src/network/alchemy_snapshot_emit.rs:218-245` 的真实 ECS 发送函数 `send_session_from_furnace`，在 `Some(s)` 分支里把 `target_ticks` 写死为 `0`、`temp_target` 写死为 `0.0`、`temp_band` 写死为 `0.0`、`qi_target` 写死为 `0.0`、`stages` 写成 `vec![]`；只有 `elapsed_ticks` / `temp_current` / `qi_injected` / `interventions_recent` 取自真实 session。与之相对，mock 通路 `mock_session()` 明明发送了完整目标值（同文件 `121-145` 行），说明 wire/schema/客户端消费端都支持这些字段，问题在真实 emit 缺字段，不在协议能力。
- **活跃可达性**：这不是 dormant mock。`BONG_ALCHEMY_JOIN_MOCKS` 默认关闭（`alchemy_snapshot_emit.rs:1-10,31-36,79-89`），正常玩家实际走的是 `client_request_handler.rs` 中 `handle_alchemy_open_furnace` / `handle_alchemy_ignite` / `handle_alchemy_intervention` / `handle_alchemy_feed_slot` / `handle_alchemy_take_back` 的重推路径，都会调用 `send_session_from_furnace`（`11349-11351`、`11518-11519`、`11447`、`11705`、`11894-11895`）。
- **客户端直接信任这些字段**：`client/src/main/java/com/bong/client/hud/AlchemyProgressHudPlanner.java:40-58,71-80` 用 `targetTicks` 算进度、用 `tempTarget/tempBand` 算温度条颜色；`client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:482-530` 直接把 `targetTicks` / `tempTarget` / `qiTarget` 画进文案，并且用 `stages` 驱动中途投料槽闪烁。server 发 0/空，client 就稳定渲染 0/空，不存在二次补算。
- **与原始设计相悖**：`docs/finished_plans/plan-alchemy-v1.md:310-314` 明确把中列定义为“状态行 + 进度条 + 温度条 + qi 条 + 干预 log”，`server/src/schema/alchemy.rs:176-190` 也把这些目标字段列为正式 `AlchemySessionDataV1` 契约，不是可选 embellishment。

## 修复方向

- `send_session_from_furnace` 需要拿到 `RecipeRegistry`（或等价 recipe lookup）并按 `session.recipe` 查配方，把 `fire_profile.target_duration_ticks / target_temp / tolerance.temp_band / qi_cost` 填入 payload。
- `stages` 需要由 `recipe.stages` + `session.staged.completed_stages/missed_stages` 生成 `AlchemyStageHintV1`，让 client 能恢复投料窗提示。
- 建议补 server 测试或 schema/emit pin：真实 session payload 不得再出现“active=true 但目标字段全 0、stages 空”的回归。

## 接入面与收口决议

- 主修复修改 server 的真实 session snapshot 构造与五个现有重推调用点；wire schema 与
  配方 JSON 不变。评审返工补入生产 protobuf 对拍、Fabric handler/store/HUD 回归，以及
  旧/缺省 wire 的 fail-closed 兼容门，但不让 client 自行重建或伪造配方目标。
- 从 `send_session_from_furnace` 抽出可纯测的 snapshot builder，并显式接收
  `RecipeRegistry`。调用点为 open furnace、ignite、intervention、feed slot、take back。
- 已知配方的目标值只取权威 `Recipe.fire_profile`：
  `target_duration_ticks`、`target_temp`、`tolerance.temp_band`、`qi_cost`，禁止复制常数。
- 每个 `RecipeStage` 映射为一个 `AlchemyStageHintV1`；`summary` 按配方声明顺序拼成
  `material×count + material×count`，`completed` / `missed` 按 stage index 从 session
  集合读取。空材料 stage 允许空 summary，不伪造提示。
- 空炉继续发送 `active=false`、`recipe_id=None` 的“未起炉”快照，用于清除旧 HUD。
- 已结束但尚在炉组件中的已知 session 发送 `active=false`，同时保留真实配方目标与阶段
  状态，避免边界快照退回零目标。
- session 指向未知配方时 fail closed：记录 warning，并发送 `active=false` 的显式
  “丹方数据缺失”快照，保留 recipe id 供诊断，但不再伪装为 `active=true + 零目标`。
- 本修复只恢复既有 HUD 数据，不新增炼丹招式/技能，不触及真元转移、配方数值、A/V
  资产或 `docs/worldview.md`。

## 实施阶段

- [x] P0：加入修复前失败的 snapshot builder 契约测试，锁定真实目标值与阶段提示。✅ 2026-07-15
- [x] P1：接通 `RecipeRegistry`，实现已知配方 snapshot 与未知配方 fail-closed。✅ 2026-07-15
- [x] P2：更新五个真实调用点，完成定向测试与 server 完整门禁。✅ 2026-07-15
- [x] P3：同步最新主线、对抗 validator、校正归档后的 Finish Evidence。✅ 2026-07-17

## 验收与测试矩阵

| 场景 | 必须断言 |
|---|---|
| 已知 active session | `active=true`；四个目标字段等于 registry 配方；运行态字段保持 session 值 |
| 多材料 stages | stage 数量/顺序、`at_tick`、`window`、`material×count + ...` summary 全匹配 |
| completed / missed | 对应 stage index 的两个布尔值分别准确，未命中 stage 均为 false |
| 空炉 | `active=false`、`recipe_id=None`、零运行态、空 stages、状态“未起炉” |
| 未知 recipe | `active=false`、保留 recipe id、状态“丹方数据缺失”，不得发 active 零目标 |
| finished session | `active=false`，但已知 recipe 的目标字段与 stages 仍完整 |
| 调用接线 | open / ignite / intervention / feed / take-back 均传入同一权威 registry |
| 旧/缺省 active wire | recipe 缺失/blank 或 `target_ticks <= 0` 时 fail closed；保留诊断字段，不渲染 `炼制 0%` |
| direct-store 绕过 | `Snapshot(active=true)` 仍须服从非空 recipe + 正 target 不变量，HUD 不得绕过 handler 防线 |
| 跨层协议 | Rust 生产 proto → Fabric handler/store → HUD planner；Python bot decoder 完整正向样本 |
| 完整门禁 | server fmt/clippy/test、Java 17 client test/build、Python protocol、真实炼丹 bot 场景 |

## 非目标

- 不改变炼丹配方目标值、阶段窗口或结果算法。
- 不让 client 从 recipe book 重建 server snapshot；兼容层只把不完整 active wire 降为 inactive，
  保留原字段用于诊断，禁止伪造 target/sentinel。
- 不顺手重构 `client_request_handler.rs` 的大型 dispatch 或其它炼丹 freshness/背包逻辑。

## 两轮反方裁决摘要

### Round 1

- **反方说法**：这可能只是 alchemy mock / vertical slice 残留，正常 gameplay 不会走到。
- **裁决**：不成立。mock 只在显式设置 `BONG_ALCHEMY_JOIN_MOCKS=1` 时给 joined client 推送；默认联机路径完全由 `handle_alchemy_open_furnace`、`handle_alchemy_ignite`、`handle_alchemy_intervention`、`handle_alchemy_feed_slot`、`handle_alchemy_take_back` 调 `send_session_from_furnace` 驱动。也就是说，玩家平时真正使用丹炉时看到的正是这条零目标值链。

### Round 2

- **反方说法**：就算 server 不发目标值，client 也许能从 recipe book 或本地硬编码自行补齐，所以不一定是 player-facing bug。
- **裁决**：不成立。HUD planner 和 `AlchemyScreen.refreshSessionText()` 都只读 `AlchemySessionStore`；它们没有从 `RecipeScrollStore` 反查目标值，也没有本地按 `recipe_id` 重建 `target_ticks/temp_target/qi_target/stages` 的逻辑。`refreshStageFlash()` 更是完全依赖 `s.stages()`。因此 server 发 0/空会稳定导致进度、温控和阶段提示全部失真，属于直接可见的 gameplay UI bug。

## 审计结论

- 主题去重：未重复 dandao mutation、voidaction target lock、processing deadpath、movement dash reject。
- 置信度：高。证据链覆盖 schema 定义、真实 emit、活跃调用点、client 消费点、原 plan 设计目标，且两轮反证均未推翻 player-facing 结论。

## Finish Evidence

### 落地清单

- `server/src/network/alchemy_snapshot_emit.rs` 的 `build_session_data` 从真实
  `RecipeRegistry` 构造完整 session snapshot：恢复时长、温度目标/容差、注气目标、按声明
  顺序生成阶段材料摘要，并保留 completed/missed 与最近干预。
- `server/src/network/client_request_handler.rs` 的 open furnace、ignite、intervention、feed
  slot、take back 五条生产重推路径均对拍权威 registry；take-back 在炉 session 移除后仍先发
  `active=false` 的 finished guidance，而不是退化为空炉零目标。
- `server/src/network/alchemy_snapshot_emit.rs` 的测试 helper 直接调用真实 `build_session_data`
  与生产 `serialize_server_data_payload_proto`，唯一生成 active / finished 两份完整
  `ServerDataEnvelope` 字节。普通测试只在内存重建并与仓库 fixture 逐字节比较；唯一写盘入口
  `regenerate_alchemy_session_production_proto_fixtures` 显式标为 maintenance-only `#[ignore]`。
- `proto/fixtures/alchemy_session_active_v1.pb`（168 bytes，SHA-256
  `73ac18233334d751c74b497d4bce002fe857a6e9c785c0d1da926e71c6a165cd`）与
  `alchemy_session_finished_v1.pb`（166 bytes，SHA-256
  `e70f4524635785f59e0c0e9a7181b61efcd1b430d0f3b800fbf4d2df8f23df48`）是 Rust producer 与
  Fabric consumer 的同一共享契约；`proto/fixtures/README.md` 记录显式重生成命令和陈旧门禁。
- `AlchemySessionHandlerProtoWireTest` 的 active / finished 用例直接读取上述 Rust 字节，经
  `ProtoServerDataBridge` → legacy envelope parser → `AlchemySessionHandler` / store →
  `AlchemyProgressHudPlanner`；Java protobuf builder 只保留给旧/缺省 wire 的 fail-closed 用例，
  不再声称覆盖 Rust producer。
- `client/src/main/java/com/bong/client/network/alchemy/AlchemySessionHandler.java` 对旧/缺省
  active wire fail closed：recipe 缺失/blank 或 `target_ticks <= 0` 时归一为 inactive，保留原
  guidance 供诊断，不制造 sentinel。
- `AlchemySessionStore.Snapshot.isActive` 再以“非空 recipe + 正 target”做 direct-store 防御；
  `AlchemyProgressHudPlanner` 与炉内 UI 因此都不会再渲染 active `炼制 0%`。
- `scripts/bot/scenarios/production_alchemy_brew_pill.py` 走真实放炉、空炉 open、点火、错误
  数量投料、正确投料、调温/注气、take-back Perfect 结算，并断言 finished inactive session
  仍保留完整 guidance。

### 关键提交

- `30ad7653`（2026-07-15）：升格炼丹 HUD 零目标 BugFix 计划。
- `55dc5553`（2026-07-15）：加入修复前失败的 snapshot 契约；锁定目标字段归零、未知
  recipe 仍 active、finished guidance 丢失。
- `210349e2`（2026-07-15）：从权威配方恢复目标/阶段提示并完成 server fail-closed。
- `9bbda6f8`（2026-07-17）：保全评审返工现场与五条生产 handler 契约测试。
- `17b99fb7`（2026-07-17）：补强 Rust 生产 protobuf 与 Fabric proto→handler/store→HUD 回归。
- `f19c7260`（2026-07-17）：补齐 Python decoder 与真实炼丹 bot 契约。
- `cf4c3ff6`（2026-07-17）：合并 `origin/main=28cc3af4` 并在未提交 merge 树上复验受影响栈。
- `4dc5b2f3`（2026-07-17）：旧/缺省 active wire 在 Fabric 边界安全降级，不再渲染零目标。
- `245d71b5`（2026-07-17）：独立锁定 direct-store 零 target 与 blank recipe 两条 HUD 防线。
- `d8d53637` / `064ddb20`（2026-07-17）：两次仅文档证据校正；它们不是最后业务代码提交。
- `70b79e4c`（2026-07-17）：用 Rust 真实 builder + 生产 encoder 生成共享 protobuf 字节，
  普通 Rust 测试锁陈旧，Fabric active / finished 测试消费同一 fixture；这是本轮最后业务代码提交。
- `d0b2005a`（2026-07-17）：双父合并 `origin/main=9d2e29d0`，在未提交 merge 树完成 server、
  Java 17、Python 与真实炼丹场景复验后落盘；这是本轮主线同步提交。
- **本节所在提交**（2026-07-17）：只原地校正既有 `## Finish Evidence`，明确区分最后业务
  代码、主线 merge、文档证据与外部最终 validator，避免用无法自证的“自身 SHA”制造循环。

### 测试结果

- Rust snapshot 定向：`cargo test network::alchemy_snapshot_emit::tests -- --nocapture` →
  7 passed / 1 maintenance-only ignored / 0 failed；覆盖 active、两份生产 fixture 逐字节陈旧门、
  空炉、未知 recipe、finished guidance。ignored writer 只有显式 `--ignored --exact` 才会写盘。
- 五条生产 handler：open / ignite / intervention / feed 的 authoritative wire 用例 4/4，
  take-back finished guidance 用例 1/1。
- Python bot protocol：`python3 scripts/bot/test_protocol.py` → 124 passed / 0 failed；炼丹专属正向
  用例 `test_alchemy_furnace_tag11_and_session_tag12` 验证 tag 12、完整目标字段、stages 与
  interventions。
- Fabric 定向（Java 17）：
  `./gradlew test --tests com.bong.client.network.alchemy.AlchemySessionHandlerProtoWireTest
  --tests com.bong.client.hud.ProcessingHudPlannerTest` → 12 passed / 0 failed；覆盖完整 active、
  finished terminal、Rust fixture 跨端链、缺省 wire、显式零 target、缺 recipe、direct-store
  零 target、blank recipe；其中炼丹 proto 类 5/5、Processing HUD 类 7/7。
- server 完整门禁（未提交 merge 树与 `d0b2005a` 内容相同，`BONG_SKIP_SKIN_PREFETCH=1`）：
  `cargo fmt --check` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test` 通过
  （lib 11745 passed / 2 ignored / 0 failed，main 11，startup 1，backpack e2e 4，doc-tests 0 failed /
  5 ignored）。
- client 完整门禁（同一 `d0b2005a` merge 树，Java 17）：`./gradlew test build` →
  BUILD SUCCESSFUL；JUnit XML 汇总 471 suites / 4110 tests / 0 failures / 0 errors / 0 skipped；产物
  `client/build/libs/bong-client-0.1.0.jar`。
- 真实目标场景：`python3 scripts/bot/run_scenarios.py --scenario production_alchemy_brew_pill`
  在当前 checkout 自起 server 后 → 1/1 PASS；点火后的 active guidance 与 take-back 后的
  finished guidance 均保留真实 target/stages。server 随后已清理，25565 释放。
- 标准全量 e2e 最新一轮为 30 total / 28 PASS / 1 SKIP / 1 FAIL；唯一失败是无关的
  `combat_weapon_equip_damage` NPC spawn 15s 超时，换全新 server 隔离复跑 1/1 PASS。
  因此不把该轮写成全量全绿，但 #1213 目标链路已有独立与全量内双重 PASS。
- 两份 fixture 在 merge 后再次对拍为 168 / 166 bytes，SHA-256 仍分别为
  `73ac18233334d751c74b497d4bce002fe857a6e9c785c0d1da926e71c6a165cd` /
  `e70f4524635785f59e0c0e9a7181b61efcd1b430d0f3b800fbf4d2df8f23df48`。

### 主线与对抗核验

- 历史第一轮在 `origin/main=28cc3af4` 上以双父 `cf4c3ff6` 合并；其后 validator 循环均绑定
  clean SHA：
  - `FAIL cf4c3ff6...`：指出缺省/旧 active wire 仍可渲染零目标，且 Finish Evidence 陈旧；
  - `FAIL 4dc5b2f3...`：确认 handler 已 fail closed，但 direct-store 防线缺独立 pin；
  - `PASS 245d71b524adf372ea919b19f1262e2f14ba39c6`：确认当时的 protobuf→handler→store→HUD
    与 direct-store 双层防线闭环，Java 17 定向 12/12 通过。
- review `issuecomment-5003480123` 随后指出两项新 blocker：Fabric 当时仍由 Java 自建正向
  protobuf，且文档把 `245d71b5` 与后续 docs-only PR HEAD 混称为最终验收 SHA。
- `70b79e4ceba38c638f7c03e4b78648b163ec798d` 闭环第一项：Rust 真实 builder / encoder 唯一
  生产共享字节，Fabric 直接消费；`d8d53637` 与 `064ddb20` 明确只是先前文档提交，不再冒充
  最后业务代码。
- fresh fetch 后 `origin/main=9d2e29d0871b004684eb4d29c11a798fc1c71d05`；
  `git merge --no-commit --no-ff origin/main` 自动合并无冲突，主线只带入 skill icon / quickslot /
  skillbar 相关 server/client 变化，未触及炼丹 fixture、proto 或本修复文件。完整受影响栈复验后
  落为双父 `d0b2005ad89d3450f6dd6690c27bf6717c76469b`。
- REBASE validator generation 4 绑定 clean `d0b2005ad89d3450f6dd6690c27bf6717c76469b`，唯一
  `FAIL` 是本节仍陈旧；未对代码、fixture 链或主线 merge 报 finding。本节所在提交只修这一项。
- **SHA 口径**：最后业务代码=`70b79e4c`；主线同步树=`d0b2005a`；Finish Evidence=
  **本节所在提交**。包含本节的完整最终 PR HEAD 将由全新只读 FINAL validator 绑定，其 PASS /
  FAIL 与远端 CI 结果写入 PR body / comment；不再修改已验证树来追写自身 SHA。

### 跨栈核验与遗留

- server：`build_session_data` / `serialize_server_data_payload_proto` /
  `assert_shared_fixture_is_current` / 五条真实 request handler。
- proto：`AlchemySession` / `AlchemyStageHint` 既有 tag；共享 active / finished Rust producer
  fixture 与 maintenance-only regeneration contract。
- client：`AlchemySessionHandler` / `AlchemySessionStore.Snapshot.isActive` /
  `ProtoServerDataBridge` / `AlchemyProgressHudPlanner` / `AlchemyScreen.refreshSessionText`。
- bot：`decode_server_data_envelope` → `_alchemy_session` / `production_alchemy_brew_pill`。
- earlier 标准全量在高 churn 场景曾触发 Valence `viewer count underflow`；同一 panic 已在锁定的
  clean `origin/main=28cc3af4` baseline worktree 独立复现，证据保存在
  `pr1213-origin-main-prefix16/server-run3.log`。它属于主线 chunk viewer 生命周期问题，禁止在
  #1213 越 scope 修复；本轮最新全量未再触发该 panic。
- 指定保留的 `server/data/backups/bong-20260717-121047.db` 与
  `bong-20260717-121055.db` 在复验后仍存在，且未进入 git diff。
- 无真元流动、配方数值、wire tag、A/V 资产变化；#1213 本身无已知功能遗留。
