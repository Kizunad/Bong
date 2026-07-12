# plan-rust-stable-clippy-baseline-v1 — Rust 1.96 stable clippy 基线恢复

> 一句话主题：仓库声明的 server 门禁 `cargo clippy --all-targets -- -D warnings` 在 Rust 1.96.1 下真实失败；不修改 toolchain、依赖或生产配置，以最小代码治理恢复完整门禁。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 在干净 `origin/main` 派生分支上复现并归类 | ✅ 2026-07-11 |
| P1 | 按 lint 类别做等价、最小代码治理 | ✅ 2026-07-11 |
| P2 | Rust 1.96.1 全 targets 与完整测试验收 | ✅ 2026-07-11 |

## 接入面

- **进料**：根 `CLAUDE.md` 声明的 server 质量门；仓库现有 `server/rust-toolchain.toml` 选择 stable 1.96.1。
- **出料**：仅收敛 `server/src/` 内被 1.96 clippy 拒绝的等价表达、测试 fixture 与 Bevy ECS 系统签名 lint 边界。
- **共享类型 / event**：不新增或改变 gameplay component、event、schema、Redis key、网络 payload。
- **跨仓库契约**：纯 server 构建卫生修复；agent/client/worldgen 均无改动。
- **worldview / qi_physics 锚点**：不涉及玩法、数值或真元流动；`qi_physics::tiandao` 仅把已有非零周期取模改为等价整数 API，守恒路径不变。

## P0 — 第一性原理验真

- 基线：`origin/main@5ffcf5458693ba040f08c9c74fab6d262ce833e0`。
- 工具链：`rustc 1.96.1 (31fca3adb 2026-06-26)`、`cargo 1.96.1`、`clippy 0.1.96`。
- 首轮 `cargo clippy --all-targets -- -D warnings` 因 lib 提前失败，报告 **69 previous errors / 55 个源文件**；修完 lib 后继续运行，揭露 **12 条 test-only lint**。完整真实基线为 **81 条**，不是可忽略的偶发失败。

### lib 69 条分类

| lint | 数量 |
|---|---:|
| `manual_is_multiple_of` | 39 |
| `derivable_impls` | 14 |
| `unnecessary_sort_by` / `manual_option_zip` / `manual_checked_ops` | 各 3 |
| `needless_borrow` | 2 |
| `useless_conversion` / `unnecessary_cast` / `type_complexity` / `too_many_arguments` / `iter_kv_map` | 各 1 |

### test-only 12 条分类

- `field_reassign_with_default` 6；`manual_is_multiple_of` 2。
- `unnecessary_cast`、`unwrap_or_default`、`needless_question_mark`、`useless_vec` 各 1。

## P1 — 最小正确治理

- 39 处生产代码周期/概率取模改为 `is_multiple_of`；逐一核对可变除数均保留 `0` 禁用 guard 或 `.max(1)`，无零除语义漂移。
- 14 个枚举手写 `Default` 改为 `#[derive(Default)] + #[default]`，原默认变体不变。
- 平均值、排序、Map values、Option 组合、无效借用/转换改用标准库直接表达；排序方向、tuple 顺序、空样本结果保持原契约。
- Bevy ECS 的独立 system params 不为 lint 强行重构调度语义：一处函数级 `too_many_arguments` 定点说明；复杂 Query 用局部 type alias 收敛。
- 12 条 test-only lint 以 struct update、数组、`div_ceil`、直接 SQLite row getter 等等价 fixture 改写清理。
- 验证发现 Clippy 1.96 对两处 `manual_option_zip` 自动建议会生成未定义变量 `f`；拒绝盲用坏建议，按原 freshness→profile 依赖关系手工等价重写并重新编译。
- 未修改 `.gitignore`、`Cargo.toml`、依赖版本、`rust-toolchain.toml` 或任何生产配置。

## P2 — 验收

- `cargo fmt --check`：PASS。
- `cargo clippy --all-targets -- -D warnings`：PASS，0 warning / 0 error。
- 最终 `git fetch origin && git merge origin/main` 同步至 `origin/main@3c8bf925`；主线带入 `server/src/cmd/dev/realm.rs` 变化后，重新执行完整 server 门禁仍全绿。
- `cargo test`：PASS：
  - lib：11156 passed / 0 failed / 1 ignored；
  - main：11 passed；`full_app_startup`：1 passed；`tarkov_backpack_p0_e2e`：4 passed；
  - doc tests：0 failed / 5 ignored；合计 **11172 passed / 0 failed / 6 ignored**。

### 2026-07-12 PR #1170 closeout 复验

- 先后普通合并 `origin/main@8baba137` 与 `origin/main@f8b4ab11`；第二次同步只带入 client/docs，合并前后的 `server/` tree 均为 `5a416df62205b09f3f3bb3e62a07cad3005f96f9`。
- 在最新组合树上重新运行 Rust 1.96.1 全 targets clippy，发现并修复主线新增的 **13 条真实 lint**：`doc_lazy_continuation` 3、`too_many_arguments` 2、`drop_non_drop` 4、`needless_question_mark` 1、`implicit_saturating_sub` 1、`field_reassign_with_default` 2。
- 生产代码仅对两个独立 Bevy ECS system params 增加函数级定点 lint 边界；其余均为文档结构与 test fixture 等价清理，不改变 gameplay、schema 或真元流动。
- 最终组合树重新执行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`：全部 PASS，合计 **11299 passed / 0 failed / 6 ignored**。
- 风险匹配测试逐项 PASS：`quick_slot_bind_clears_only_the_old_auto_mirrored_item`、`v34_migration_creates_pending_inflow_runtime_account_table`、`cultivation_detail_all_20_meridians`、`heartbeat_tick_keeps_pseudo_vein_state_zone_and_ledger_in_lockstep`、`restored_pseudo_vein_first_tick_returns_dynamic_zone_balance_to_pending_pool`。
- 旧 `/review` 的 A/B/C/D 四路 reviewer 均为 `confidence: 0`；其唯一“finding”指向 `.github/scripts/review.mjs:0`，证据均为 `hlool` 无可用 `gpt-5.6-sol` 通道的 HTTP 503，未包含任何 PR 代码路径或行为 finding。
- 首轮 fresh read-only `gpt-5.6-sol high` validator（`019f547f-01f6-74c0-a7e8-9c0fb257836f`）绑定 `e4d79143` 从零复核后给出 FAIL：代码、工具链、clippy 与 review 证据均无问题，唯一失败是 `poi_novice` 的 10 秒墙钟断言在并发负载下耗时 12.218 秒。
- `c2a19353` 抽出共享采样 helper，生产路径仍保留 500,000 次上限；测试改为断言确定性的实际尝试次数。全阻塞与 POI 铺满两条定向测试分别约 0.22 秒、0.14 秒通过。
- 后续完整门禁又证实 `fauna::migration` 的 5 ms 墙钟阈值会在并发负载下以 5.511743 ms 假红；`39d7592e` 改为确定性状态契约，覆盖非调度 tick 不移动、调度 tick 全部移动、每只最多一步且 Y 不变，定向测试约 0.02 秒通过。
- 在 `39d7592e` 上重新执行完整 server 三门禁：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全部 PASS；lib 11283 passed / 1 ignored，main 11 passed，两个 integration target 共 5 passed，doc tests 5 ignored，合计 **11299 passed / 0 failed / 6 ignored**。
- 最后普通合并 `origin/main@f6322e6a`；增量仅涉及 client 与另一份归档 plan，合并前后 `server/` tree 均为 `fcff3848201755900b433d9ddd09d87fb7414df2`，上述 Rust 门禁仍绑定最终组合树。

## Finish Evidence

### 落地清单

- 周期/概率判断：`server/src/{botany,combat,cultivation,dandao,fauna,gathering,identity,mineral,network,npc,qi_physics,shelflife,spiritwood,world}/`。
- 默认值派生：`server/src/{combat,cultivation,fauna,movement,npc,schema,social,world,zhenfa}/`。
- 标准库组合与测试 fixture：`server/src/{alchemy,body_plan,botany,cmd,combat,cultivation,inventory,lingtian,network,npc,persistence,social,world}/`。

### 关键 commit

- `27a356ca`（2026-07-11）：统一 Rust 1.96 整数周期判断。
- `b9b3938d`（2026-07-11）：派生 14 个枚举默认值。
- `5e29e9bb`（2026-07-11）：收敛 lib 剩余标准库与 Bevy lint。
- `db50bc17`（2026-07-11）：清理全 targets 的 12 条 test-only lint。
- `a07c5516`（2026-07-11）：同步 `origin/main@3c8bf925`，带入 server 变更后重新验收。
- `c7f7155c`（2026-07-12）：收敛最新主线生产 lib 的 5 条 Rust 1.96 lint。
- `a8c26295`（2026-07-12）：清理最新主线全 targets 的 8 条 test-only lint。
- `af4c854e`（2026-07-12）：同步 `origin/main@f8b4ab11`，确认 `server/` tree 未变并重新执行完整门禁。
- `c2a19353`（2026-07-12）：以确定性尝试次数契约替代 POI 散布墙钟门禁。
- `39d7592e`（2026-07-12）：以确定性状态契约替代兽潮迁移墙钟门禁。
- `9f6a5632`（2026-07-12）：同步 `origin/main@f6322e6a`，确认最终 `server/` tree 未变。

### 测试结果

- Rust 1.96.1 下 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 在最终 `server/` tree 上全绿；PR closeout 最终计数 11299 passed / 0 failed / 6 ignored。五组风险匹配测试与两项负载敏感测试的确定性替代契约均单独复跑 PASS。

### 跨仓库核验

- server：完整门禁恢复；agent/client/worldgen 无 diff、无契约变更，不需要跨栈构建。

### 遗留 / 后续

- 无功能遗留。首轮 validator 揭露的两项负载敏感墙钟门禁均已改为确定性契约；PR closeout 最终 evidence commit 后，仍须由另一名 fresh 无上下文 read-only `gpt-5.6-sol high` validator 绑定该 SHA 做最终对抗验证，不能用首轮审查代替。
