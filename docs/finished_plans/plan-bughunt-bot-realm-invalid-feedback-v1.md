# plan-bughunt-bot-realm-invalid-feedback-v1

> **已完成（2026-07-10，由 bughunt skeleton promotion）**。一句话主题：bot-e2e live run 发现 `/realm set <非法 id>` 被命令解析层拒绝后没有玩家可见 chat 反馈；玩家只看到命令静默失败，无法从客户端判断错误是拼写、境界 id 还是命令链路丢包。

> 立项动机：PR #978 的 `cultivation_realm_qi` 场景曾按“dev 命令非法值也应有反馈”建模，但 CI live bot 证明 `/realm set bot_e2e_no_such_realm` 10 秒内无任何 chat。`/qi set -1` 已有明确 `[dev] qi set rejected: value must be finite >= 0`，realm 命令在同类 dev 调试体验上缺一条可观测错误面。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `/realm set` 非法 id 无玩家可见反馈 | fix_pr | ✅ 2026-07-10 |

## P0 — `/realm set` 非法 id 无玩家可见反馈

- **#1 major（fix_pr）**：`server/src/cmd/dev/realm.rs` 的非法 id 在 `RealmArg::parse_arg` 中返回 `CommandArgParseError::InvalidArgument`，但 live bot 没收到任何玩家 chat。
  - 成功路径在 `handle_realm` 内才执行 `client.send_chat_message(format!("[dev] realm set {prev:?} -> {id:?}"))`。
  - 非法 id 无法生成 `CommandResultEvent<RealmCmd>`，因此不会进入 `handle_realm`，也没有本命令自己的拒绝反馈分支。
  - Valence / Brigadier 解析层是否会把 `InvalidArgument` 自动回传给客户端，目前 live 证据为“没有玩家可见 chat”。
  - 结果：玩家输入 `/realm set bot_e2e_no_such_realm` 时，客户端侧只看到静默失败；bot 黑盒也无法区分 parser 拒绝、命令包丢失、权限/注册问题。

## 玩家影响

- dev / QA / bot 调试时，合法 realm id 拼错会表现成无反馈，排查成本高。
- 同类 `/qi set -1` 已有明确 `[dev] qi set rejected: value must be finite >= 0`，两条修炼 dev 命令反馈体验不一致。
- bot e2e 无法把“非法 realm id 应拒绝”写成稳定黑盒覆盖，因为当前没有可观测事件可断言。

## 建议修法

- 让 `/realm set` 的非法 id 产生玩家可见反馈，例如：
  - 保持当前 parser 类型不变，在命令框架统一错误回传层补 chat；
  - 或把 realm id 先按 String 接入命令，进入 handler 后调用 `parse_realm`，失败时 `send_chat_message("[dev] realm set rejected: unknown realm ...")`。
- 反馈文案应包含非法输入和允许值：`awaken|induce|condense|solidify|spirit|void`。
- 修法不能改变合法 `/realm set induce` 的成功反馈契约。

## 测试抓手

- server 侧补命令 handler/解析集成测试：非法 id 时玩家收到拒绝反馈，合法 id 仍修改 realm。
- bot-e2e 后续可恢复非法 realm id 分支：发送 `/realm set bot_e2e_no_such_realm` 后断言拒绝 chat。
- 保留 `/qi set -1` bot 覆盖作为同类 dev 命令非法值反馈的现有基线。

## 反方问题

1. “Brigadier 客户端本应阻止非法输入，所以不需要 server chat。”
   反证：bot 和复制粘贴命令都能把非法字符串发到 server；黑盒玩家仍需要可见反馈。
2. “这是 dev-only 命令，不影响正式玩法。”
   反证：dev 命令是 bot-e2e 铺垫和 QA 调试入口，静默失败会直接污染 e2e 诊断质量。

## 审计来源

bot 覆盖发现，后续 fix。来源为 PR #978 `bot-e2e` live run：`cultivation_realm_qi` 中 `/realm set bot_e2e_no_such_realm` 10 秒超时无 chat；同 run 中其他场景通过，`cultivation_breakthrough` 通过，说明 bot 连接和修炼基础链路本身可用。

## Finish Evidence

### 落地清单

- **P0 server 命令链路**：`server/src/cmd/dev/realm.rs` 将 realm id 作为 `String` 接入命令图，在 `handle_realm` 内调用 `parse_realm`；非法 id 通过 `Client::send_chat_message` 返回原输入与 `awaken|induce|condense|solidify|spirit|void`，合法 `realm set induce` 保持原成功反馈和状态变化。
- **P0 server 协议回归**：同模块测试以真实 `CommandExecutionC2s` 驱动命令管理器并解码 `GameMessageS2c`；合法路径锁定 `Awaken -> Induce`，非法路径从非默认 `Realm::Induce` 出发锁定状态不变。
- **P0 bot 黑盒回归**：`scripts/bot/scenarios/cultivation_realm_qi.py` 先执行合法 `realm set induce`，再发送非法 id 并精确断言玩家 chat；`scripts/bot/test_protocol.py` pin 该场景可被 runner 自动发现。

### 关键 commit

- `10f5c685` · 2026-07-10 · 将 bughunt skeleton 提升为 active plan。
- `d332f41a` · 2026-07-10 · 修复 realm 非法境界反馈并补真实 C2S/S2C 集成测试。
- `e5785f0e` · 2026-07-10 · 恢复 bot realm 非法反馈黑盒回归与 discovery pin。
- `247ac07c` · 2026-07-10 · 以非默认 `Realm::Induce` 锁定非法输入不修改既有境界。

### 测试结果

- `env BONG_SKIP_SKIN_PREFETCH=1 cargo test --manifest-path server/Cargo.toml cmd::dev::realm::tests`：4 passed，0 failed。
- `python3 scripts/bot/test_protocol.py`：47 passed，0 failed。
- `cd server && cargo fmt --check`、`git diff --check`：通过。
- PR #1151 CI run `29088759260`（HEAD `f33860b4`）：client、schema、agent、server 全量 `cargo test`、smoke 均通过；`cultivation_realm_qi` 明确 PASS（0.4s）。Bot e2e 总计 21/22，唯一失败为本 plan 范围外的 `production_forge_station_real_place` 锻造快照超时。
- 较早 CI run `29064915661` 同样记录 `cultivation_realm_qi` PASS；两轮均证明 realm 目标场景稳定通过，整体红灯与该修复无因果关系。

### 跨仓库核验

- **server**：`RealmCmd::Set { raw }`、`ALLOWED_REALM_IDS`、`handle_realm`、`CommandExecutionC2s` 与 `GameMessageS2c` 均可 grep 命中，构成命令输入到玩家 chat 的真实链路。
- **bot harness**：`cultivation_realm_qi.run` 与 `RunnerLogicTest::test_discover_scenarios_finds_committed_set` 均可 grep 命中，CI 的 `run_scenarios.py --all` 会消费该场景。
- **agent / client**：本修复沿用原版命令执行包与玩家 chat，不新增 schema、Redis key 或 Fabric 客户端协议，故无需跨端代码变更。

### 遗留 / 后续

- `production_forge_station_real_place` 的 forge session 快照超时连续两轮导致整体 Bot e2e 红灯，属于炼器场景范围，不在本 plan 内处理。
- CodeRabbit 本轮因 review 服务失败/额度状态未给出有效审查；realm 最终提交已由独立 `fork_context:false`、`gpt-5.6-sol` Ultra read-only validator 对抗验证为 PASS。
