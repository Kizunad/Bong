# plan-server-cmd-system-v1

把 server 端 chat 命令从 hard-parser 迁移到 Valence 原生 brigadier 命令系统：`!xxx` dev 命令全部改用 MC 原生 `/` 命令，享受客户端 Tab 自动补全；命名扁平（无 `/bong` / `/dev` 前缀）。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | Valence 命令框架接入（最小 `/ping` 验证 Tab 补全） | ⬜ |
| P1 | 11 条 `!xxx` dev 命令迁移到扁平 `/xxx` | ⬜ |
| P2 | `/bong combat\|gather\|breakthrough` 产品命令迁移（可选/可拆） | ⬜ |
| P3 | 测试饱和 + brigadier 命令树 e2e | ⬜ |
| P4 | 文档同步（CLAUDE.md / 引用了 `!xxx` 的 plan） | ⬜ |

## 调研结论（前置事实）

- Valence pin rev `2b705351` (2026-01-15) 已含 `valence_command` + `valence_command_macros`，命令系统稳定
- 客户端 Tab 补全由 Valence 自动通过 `Commands` packet (0x0E) 推送，**client / agent 零改动**
- 现有 `/bong combat` 与 11 条 `!xxx` 都走硬解析（`server/src/network/chat_collector.rs:185-514`），无 brigadier 树
- 11 条 dev 命令的依赖（`debug_combat_tx` / `TsySpawnRequested` event / `PendingScenario` resource / `ZoneRegistry` / `TerrainProvider` / `PlayerStatePersistence`）已是 Bevy event/resource 形式，handler 适配成本低

---

## P0 — Valence 命令框架接入

最小可用：注册 `/ping` 验证 brigadier 树发送和 client Tab 补全工作。

**交付物**：
- `server/Cargo.toml`：加 `valence_command` + `valence_command_macros` 依赖（同 workspace pin rev `2b705351`）
- `server/src/cmd/mod.rs`：新模块，挂 `CommandHandlerPlugin` 到 Bevy `App`
- `server/src/cmd/ping.rs`：`#[derive(Command)] enum PingCmd { #[paths("ping")] Ping }`，handler 回 chat "pong"
- `server/src/main.rs` 或主 `App` builder 注册新模块
- 单测：`cmd::ping::tests` ≥ 2 个（注册成功 / handler 触发）

**验收**：
- `cargo test -p bong-server cmd::` 通过
- 手测：MC 客户端 `/p<TAB>` 能看到 `/ping` 候选并补全；执行后聊天栏显示 "pong"

---

## P1 — dev 命令迁移

把 `chat_collector.rs:215-514` 的 11 条 `!xxx` 全部迁到 Valence command，命名扁平。

**命名映射**（连字符 → 下划线，brigadier literal 限制）：

| 旧 | 新 | 参数 |
|----|----|----|
| `!spawn` | `/spawn` | — |
| `!top` | `/top` | — |
| `!zones` | `/zones` | — |
| `!gm <c\|a\|s>` | `/gm <mode>` | enum |
| `!health set <n>` | `/health set <n>` | `f32` |
| `!stamina set <n>` | `/stamina set <n>` | `f32` |
| `!tptree <spirit\|dead>` | `/tptree <tree>` | enum |
| `!tpzone <name>` | `/tpzone <zone>` | `String` |
| `!shrine <set\|clear>` | `/shrine <action>` | enum |
| `!wound add <part> [severity]` | `/wound add <part> [severity]` | enum + `Option<f32>` |
| `!tsy-spawn <family_id>` | `/tsy_spawn <family_id>` | `String` |
| `!npc_scenario <type>` | `/npc_scenario <type>` | enum |

**实现结构**：
- `server/src/cmd/dev/mod.rs`：注册聚合
- `server/src/cmd/dev/{spawn,top,zones,gm,health,stamina,tptree,tpzone,shrine,wound,tsy_spawn,npc_scenario}.rs`：每条命令一个文件，含 `#[derive(Command)] enum XxxCmd` + 一个 `fn handle_xxx(...)` Bevy system
- handler 复用现有 events/resources：
  - `DebugCombatCommand` event（`/wound` / `/health` / `/stamina` / `/shrine`）
  - `TsySpawnRequested` event（`/tsy_spawn`）
  - `PendingScenario` resource（`/npc_scenario`）
  - 直接改 `Position` / `GameMode` component（`/spawn` / `/top` / `/gm` / `/tptree` / `/tpzone`）
  - `ZoneRegistry` 查询（`/zones` / `/tptree` / `/tpzone`）

**清理**：
- 删除 `chat_collector.rs:215-514` 的 `try_handle_dev_command()` 整个函数
- `chat_collector.rs:228` 的 `starts_with('!')` 早期返回逻辑改为：以 `!` 开头的消息直接丢弃，并提示玩家 "`!` 命令已迁至 `/`，使用 Tab 补全"
- `try_handle_dev_command` 的 9 个参数依赖从 `collect_player_chat()` 签名移除

**测试**：
- 每条命令 ≥ 4 个单测覆盖：happy / 边界（参数缺失、超范围）/ 错误（unknown enum、parse fail）/ 状态前置
- 共 ≥ 44 单测

**验收**：
- 11 条命令全部跑通 Tab 补全（手测）
- `cargo test cmd::dev` 全绿
- `chat_collector.rs` 行数从 ~520 降到 ~200（移除 hard parser）

---

## P2 — 产品命令迁移（可选/可拆）

`/bong combat|gather|breakthrough` 也迁到 valence_command，保留命名（`/bong combat` 走 subcommand 树）。

**交付物**：
- `server/src/cmd/gameplay/mod.rs` + `combat.rs` / `gather.rs` / `breakthrough.rs`
- 删除 `parse_gameplay_action()` (`chat_collector.rs:185-212`)
- 删除 `GameplayAction` 入队的硬路径，改成 valence command handler emit 同等 event
- 测试：3 条命令各 happy + 错误 case ≥ 8 单测

**说明**：本阶段独立可裁剪。如想优先把 P0/P1 落地，P2 可拆成独立后续 plan（如 `plan-gameplay-cmd-v1`）。归档前若 P2 ⬜，按需删除该阶段。

---

## P3 — 测试饱和 + e2e

**单元层**：
- 每条命令 happy / 边界（empty / max / boundary off-by-one）/ 错误分支 / enum variant 全覆盖（CLAUDE.md "饱和化测试" 原则）
- `server/src/cmd/dev/<command>.rs` 内联 `#[cfg(test)] mod tests`

**协议层 e2e**：
- `server/tests/cmd_brigadier_tree.rs`：构造一个 mock client session，发 `CommandSuggestionsRequest` (0x09)，断言 server 回 `CommandSuggestionsResponse`，候选清单包含全部 dev 命令名
- 断言 brigadier 树深度、参数类型对照（dev 命令树 frozen fixture）

**Pin 测试**：
- `server/src/cmd/registry_pin.rs`：命令名清单 frozen 数组，加新命令必须同改 fixture（CLAUDE.md 提到的 schema 等同 pin 模式）

**验收**：
- `cargo test --all-targets cmd::` 全绿
- 命令清单变更触发 fixture diff，PR review 必须更新

---

## P4 — 文档同步

- `CLAUDE.md`：搜 `!shrine` / `!spawn` / `!gm` 等所有 `!xxx` 提及，全部替换为 `/xxx`
- `docs/plan-tsy-zone-v1.md`（active）：`!tsy-spawn` 引用 → `/tsy_spawn`
- `docs/finished_plans/` 中含 `!wound` / `!health` / `!stamina` 的 plan：在文末加一行迁移注释，**不修改正文**（已归档 plan 不重写历史）
- `README` / 其他用户文档（如有）：同步更新

---

## 跨仓库影响

| 仓库 | 改动 |
|----|----|
| `server/` | 新增 `src/cmd/`、删 `chat_collector.rs:215-514`、改 `Cargo.toml` |
| `agent/` | 无 |
| `client/` | 无（Fabric 微端通过原版协议自动收到 brigadier 树） |
| `worldgen/` | 无 |

## 风险 / 开放点

- **`valence_command` 标 `0.2.0-alpha.1`**：版本号有 alpha 但 example 完整、生产可用。若实施中发现 API 不稳，回退方案是手写 `Commands` packet (0x0E) + 命令树（工作量翻倍）
- **handler 拿不到 `&mut World`**：Valence command handler 是 Bevy system 形式，依赖 `SystemParam` 注入资源/事件 writer。已现有 `DebugCombatCommand` 等 event-based 模式可复用，无技术阻碍
- **`/tpzone` 动态候选**：当前 `/tpzone` 候选来自 `ZoneRegistry`（运行时数据），不是静态 enum。若 `valence_command` 不支持动态 suggestion provider，候选退化成 `String` 自由输入（不影响功能，仅 Tab 体验降级）
- **P2 是否纳入**：取决于是否一次性清理硬解析。建议 P0/P1 完成验收后再决定

## Finish Evidence

### 落地清单

- **P0 Valence 命令框架接入**：`server/Cargo.toml` 已接入 `valence_command` / `valence_command_macros`（rev `2b705351`）；`server/src/cmd/mod.rs` 统一注册命令模块；`server/src/cmd/ping.rs` 提供 `/ping` 与 pong handler；`server/src/main.rs` 已调用 `cmd::register(&mut app)`。
- **P1 dev 命令迁移**：11 条 legacy `!xxx` 已迁移到扁平 slash 命令，文件为 `server/src/cmd/dev/{spawn,top,zones,gm,health,stamina,tptree,tpzone,shrine,wound,tsy_spawn,npc_scenario}.rs`；`server/src/cmd/dev/mod.rs` 聚合注册；`server/src/network/chat_collector.rs` 已移除硬解析 dev handler，仅丢弃已知 legacy `!` 命令并提示迁移。
- **P2 产品命令迁移**：`server/src/cmd/gameplay/mod.rs` 注册 `/bong combat <target> <qi_invest>`、`/bong gather <resource>`、`/bong breakthrough`；`server/src/cmd/gameplay/{combat,gather,breakthrough}.rs` 复用 `GameplayActionQueue` 语义。
- **P3 测试饱和 + 命令树冻结**：`server/src/cmd/registry_pin.rs` 冻结 root command 清单与 executable path fixture；`server/src/cmd/mod.rs` 测试覆盖 Valence `CommandRegistry` 与 `CommandTreeS2c` packet root literals；`server/src/cmd/dev/*.rs` 内联测试覆盖 parser、状态前置和 handler 副作用。
- **P4 文档同步**：`CLAUDE.md` / `README.md` 无 `!xxx` 残留；`docs/plan-tsy-zone-v1.md` 已不再是 active plan；已按授权在 `docs/finished_plans/{plan-tsy-v1.md,plan-tsy-zone-v1.md,plan-tsy-zone-followup-v1.md,plan-tsy-container-v1.md}` 追加 `!tsy-spawn` → `/tsy_spawn <family_id>` 迁移注释，正文历史记录不重写。

### 关键 commit

- `162bc974` (2026-04-28) — `feat(server): 迁移调试命令到原生命令树`，主实现落地 `server/src/cmd/`、`server/Cargo.toml` 与 `chat_collector` 清理。
- `475bf76a` (2026-05-01) — `test(cmd): 冻结 brigadier 命令树覆盖`，补齐 dev 命令测试和 executable path fixture。
- `4ed0ed53` (2026-05-01) — `docs(cmd): 标注 TSY 调试命令迁移`，给归档 TSY plan 追加迁移注释。
- `9b393dc9` (2026-05-01) — `fix(cmd): 限定命令树 fixture 为测试常量`，修复 clippy dead_code。
- `dbea9ea9` (2026-05-01) — `test(cmd): 覆盖 brigadier 命令树 packet`，补 `CommandTreeS2c` 协议层 root literal 覆盖。

### 测试结果

- `cd server && cargo test cmd::` — 69 passed；覆盖 `/ping`、dev 命令、`/bong` gameplay 命令、registry pin 与 packet root literal。
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` — 通过；最终 `cargo test` 1872 passed, 0 failed, 0 ignored。

### 跨仓库核验

- **server**：`PingCmd` / `SpawnCmd` / `TopCmd` / `ZonesCmd` / `GmCmd` / `HealthCmd` / `StaminaCmd` / `TptreeCmd` / `TpzoneCmd` / `ShrineCmd` / `WoundCmd` / `TsySpawnCmd` / `NpcScenarioCmd` / `BongCmd`；`registry_pin::COMMAND_NAMES`；`registry_pin::COMMAND_TREE_PATHS`；`CommandTreeS2c` packet root literal 测试；`LEGACY_BANG_COMMANDS` migration notice。
- **agent**：无代码改动；命令迁移不改变 Redis IPC schema 或 agent contract。
- **client**：无代码改动；Fabric 微端通过原版协议消费 Valence `Commands` packet，Tab 补全无需自定义 payload。
- **worldgen**：无代码改动。

### 遗留 / 后续

- 本自动消费环境未启动真人 MC 客户端做 `/p<TAB>` 手测；已用 Valence `CommandRegistry` 和 `CommandTreeS2c` packet 测试锁定服务端命令树输出。
- `/tpzone <zone>` 仍为 `String` 自由输入，未接动态 suggestion provider；这与本 plan 风险项一致，不影响命令执行。
