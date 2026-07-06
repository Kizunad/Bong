# plan-bughunt-bot-realm-invalid-feedback-v1（骨架）

> **骨架（草案）**。一句话主题：bot-e2e live run 发现 `/realm set <非法 id>` 被命令解析层拒绝后没有玩家可见 chat 反馈；玩家只看到命令静默失败，无法从客户端判断错误是拼写、境界 id 还是命令链路丢包。

> 立项动机：PR #978 的 `cultivation_realm_qi` 场景曾按“dev 命令非法值也应有反馈”建模，但 CI live bot 证明 `/realm set bot_e2e_no_such_realm` 10 秒内无任何 chat。`/qi set -1` 已有明确 `[dev] qi set rejected: value must be finite >= 0`，realm 命令在同类 dev 调试体验上缺一条可观测错误面。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `/realm set` 非法 id 无玩家可见反馈 | fix_pr | ⬜ |

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
