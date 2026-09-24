# plan-bughunt-ac-alchemy-hud-zero-targets-v1

> **状态：finished（2026-07-19 最终验收；2026-07-15 promotion / 归档）**。来源：
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
- [x] P3：同步最新主线、对抗 validator、校正归档后的 Finish Evidence。✅ 2026-07-19

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

- `server/src/network/alchemy_snapshot_emit.rs` 的 `build_session_data` 以
  `RecipeRegistry` 为权威来源，恢复 `target_ticks`、`temp_target`、`temp_band`、
  `qi_target` 与按配方声明顺序生成的 stages；运行态的 elapsed、当前温度、已注真元、
  completed/missed stage 与最近干预仍来自真实 `AlchemySession`。未知 recipe 只发送
  `active=false` 的“丹方数据缺失”诊断快照，不再伪装成 active 零目标。
- `server/src/network/client_request_handler.rs` 的 open、ignite、intervention、feed 与
  take-back 生产路径把同一权威 registry 传给 snapshot producer。take-back 在既有结算后通过
  `send_session_from_completed_session` 推送 terminal guidance；本 plan 只核验 HUD snapshot，
  **不声明** take-back 具备可重试领取、grant-before-clear 或 exactly-once ownership。
- `server/src/network/alchemy_snapshot_emit.rs` 的 fixture helper 直接复用生产 builder 与
  `serialize_server_data_payload_proto`。普通测试只在内存重建并逐字节对拍；唯一写盘入口
  `regenerate_alchemy_session_production_proto_fixtures` 为显式 `#[ignore]` maintenance test。
- `proto/fixtures/alchemy_session_active_v1.pb` 与
  `proto/fixtures/alchemy_session_finished_v1.pb` 是 Rust producer 与 Fabric consumer 的共享字节契约；
  `proto/fixtures/README.md` 记录来源、显式重生成命令和 stale-fixture gate。
- `ProtoServerDataBridge` → `AlchemySessionHandler` → `AlchemySessionStore` 保留完整的 active 与
  terminal guidance。handler 对 recipe 缕失/blank 或 `target_ticks <= 0` 的 active wire fail closed；
  `Snapshot.isActive()` 再以非空 recipe 与正 target 形成 direct-store 第二道门。
- `AlchemySessionStore` 在 snapshot replace 后通知已打开的真实 `AlchemyScreen`；screen 订阅幂等，
  `removed()`/detach 后解除监听且不会复活。`AlchemySessionPresentationPlanner` 的顺序为
  active → authoritative terminal guidance → empty furnace → occupied/waiting，避免 empty-furnace
  snapshot 抢先吞掉随后到达的 completed snapshot。
- 炉内 screen 对 active A→B 更新实时刷新 elapsed/temp/qi、stage 状态/窗口/摘要与 intervention；
  completed/inactive snapshot 保留目标值、阶段和干预 guidance，但中央 processing HUD 隐藏。
  stage 闪烁窗口使用闭区间，completed/missed stage 不继续闪烁；terminal 的 T 操作仍发送真实
  `alchemy_take_back` 请求。
- `scripts/bot/proto_min.py`、`scripts/bot/test_protocol.py` 与
  `scripts/bot/scenarios/production_alchemy_brew_pill.py` 锁定 tag 解码及放炉、空炉 open、点火、
  投料、调温/注气、take-back 后 finished inactive guidance 的生产场景。

### 范围校正

- 历史提交 `afc344960` / `5a6651cc0` 曾把 finished-unclaimed、失败可重试与请求拒绝状态机
  混入 #1213；`087899f1a` 与 `a9fd6a81f` 以 forward-only revert 移除该实现，
  `4924d3863` 明确把丹成待领取状态机移出本 PR 范围。当前生产代码中不再存在
  `finished_unclaimed`、`has_unclaimed_finished` 及对应 feed/intervention reject symbols。
- 当前 `handle_alchemy_take_back` 会先把 session 推到 finished 并 `end_session()`，之后才尝试
  allocator/grant；因此满包、模板缺失或 allocator 缺失仍可能让炉 session 与产物同时丢失，
  无法重试。该独立 product-loss bug **没有被 #1213 修复**。
- 后续权威任务仍是未消费、未修改的
  `docs/plans-skeleton/plan-bughunt-alchemy-takeback-full-inventory-loss-v1.md`。本 PR 不 promotion、
  不归档、不修改该 skeleton，也不把其验收结果提前写入本 plan。

### 关键提交

- `30ad7653`（2026-07-15）：升格炼丹 HUD 零目标 BugFix 计划。
- `55dc5553` / `210349e2`（2026-07-15）：建立修复前契约并从权威配方恢复目标、阶段与
  unknown-recipe fail-closed。
- `17b99fb7` / `70b79e4c`（2026-07-17）：接通 Rust 生产 protobuf 字节与 Fabric
  proto→handler/store→HUD 对拍，普通测试锁 fixture 陈旧。
- `f19c7260`（2026-07-17）：补齐 Python decoder 与真实炼丹 bot 契约。
- `4dc5b2f3` / `245d71b5`（2026-07-17）：在 Fabric handler 和 direct-store 两层锁住不完整
  active wire 的 fail-closed。
- `91aff7d65` / `47763faac` / `f1f9df2f0`（2026-07-20）：接通已打开炼丹 screen 的 live
  listener，锁定 active A→B 更新、生命周期解绑与 stage 闪烁刷新。
- `087899f1a` / `a9fd6a81f` / `4924d3863`（2026-07-23）：移除误并入的丹成待领取状态机，
  将 product-loss 修复归还独立 skeleton。
- `b7cf51e87`（2026-07-23）：把 terminal screen 语义校正为 HUD 通用完成态，不再暗示
  finished-unclaimed ownership。
- `16afda490`（2026-07-23）：补强 Rust active/finished fixture 经 production bridge 到真实
  `AlchemyScreen` 的代表链回归，并保持测试 seam package-private。
- `5070fa25c`（2026-07-23）：修复 empty furnace 抢先吞 terminal guidance 及重复“已结束”文案。
- `dd9acc86a`（2026-07-23）：合并 `origin/main=0b06c4164`；该业务候选树无冲突并完成下述
  server/client/Python 门禁。本 Finish Evidence 校正提交不伪造自身 SHA。

### 测试结果

- **server 完整门禁，业务候选 `dd9acc86aedca0b81514e686e76d8523e16ddbcf`**：先在目标
  worktree 执行 `npm ci --prefix agent` 与 `npm run build -w @bong/schema` 补齐跨栈测试运行依赖，
  随后直接执行（无尾部管道）
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → **EXIT 0**。
  汇总为 lib **11875 passed / 0 failed / 2 ignored**，main **11/0/0**，
  full-app integration **1/0/0**，Tarkov integration **4/0/0**，doc tests **0 passed / 5 ignored**；
  总计 **11891 passed / 0 failed / 7 ignored**。
- **client 完整门禁，同一业务候选，Java 17.0.19**：
  `./gradlew test build --no-daemon --console=plain` → **EXIT 0 / BUILD SUCCESSFUL**；
  JUnit XML 汇总 **484 files / 4267 tests / 0 failures / 0 errors / 0 skipped**，
  Fabric GameTest **3/3 required passed**。
- **Python protocol**：`python3 -m unittest scripts.bot.test_protocol` →
  **130 passed / 0 failed**；仅有非致命 unclosed-socket `ResourceWarning`。
- 两份 fixture 仍分别为 **168 bytes** 与 **166 bytes**；SHA-256 保持
  `73ac18233334d751c74b497d4bce002fe857a6e9c785c0d1da926e71c6a165cd` 与
  `e70f4524635785f59e0c0e9a7181b61efcd1b430d0f3b800fbf4d2df8f23df48`。
- 早期 server 重跑曾分别因目标 worktree 缺 `tsx`、随后缺 `@bong/schema/dist` 失败；这些是
  真实失败而非 PASS。安装锁定依赖并构建 schema 后，以上完整门禁在同一业务 SHA 重新执行为绿。

### 跨栈核验与遗留

- server：`build_session_data` / `send_session_from_furnace` /
  `send_session_from_completed_session` / production protobuf fixture builder。
- proto：`AlchemySession` / `AlchemyStageHint` 既有字段与两份 Rust producer fixture。
- client：`ProtoServerDataBridge` / `AlchemySessionHandler` /
  `AlchemySessionStore.Snapshot.isActive` + listener / `AlchemySessionPresentationPlanner` /
  `AlchemyScreen` / `AlchemyProgressHudPlanner`。
- bot：`decode_server_data_envelope` → `_alchemy_session` /
  `production_alchemy_brew_pill`。
- 不涉及真元流动、配方数值、wire tag、资源资产或 `docs/worldview.md`；没有触碰 PR #1228。
- 唯一已知独立遗留是 take-back 失败后的 product/session loss，继续由上述未消费 skeleton 负责。
- 本节不把超时/异常 validator 写成 PASS；Finish Evidence 提交后的最终 exact HEAD 仍须重新执行
  无上下文只读 validator、GitHub E2E、独立 `/review` 与 CodeRabbit gate。任何新 SHA 都不得沿用旧结果。
