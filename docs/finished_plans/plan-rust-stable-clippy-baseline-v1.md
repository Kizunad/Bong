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

### 测试结果

- Rust 1.96.1 下 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 在同步最新主线前后均全绿；post-merge 最终计数 11172 passed / 0 failed / 6 ignored。原始日志保存在本次执行环境 `/tmp/bong-rust-1961-*.log`，不进入仓库。

### 跨仓库核验

- server：完整门禁恢复；agent/client/worldgen 无 diff、无契约变更，不需要跨栈构建。

### 遗留 / 后续

- 无功能遗留。归档 commit 产生最终 HEAD 后，由 fresh 无上下文 read-only validator 绑定该 SHA 做对抗验证；其 PASS/FAIL 作为 PR gate 证据，不能用归档前审查代替。
