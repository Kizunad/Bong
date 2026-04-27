---
name: pr-watch
description: 阻塞当前会话轮询某个 PR，直到出现新评论/review（包括 Codex、Claude bot、人类 reviewer）或超时后自动返回。用法：/pr-watch <PR#> [总分钟数] [轮询秒数]
argument-hint: <PR#> [总分钟数=9] [轮询秒数=60]
allowed-tools: Bash Read
---

# PR 新评论守望

阻塞会话轮询指定 PR 的评论/review 活动。脚本在 `.claude/skills/pr-watch/watch.sh`。

检测三类活动：
- issue comments（PR 主讨论串）
- review comments（行内 review）
- reviews（approve / request changes / comment review）

包括新增 **和** 被编辑（updated_at 变化也算）。

---

## 参数解析

`/pr-watch <PR#> [总分钟数] [轮询秒数]`

- `<PR#>` 必填，PR 编号（纯数字）
- 总分钟数：默认 9，最大单次 bash 调用 9 分钟（Claude Code 硬上限）；> 9 需链式调用
- 轮询秒数：默认 60

用户可能用自然语言说"盯 42 号 PR 20 分钟"——自己解析成 `PR=42 MINUTES=20`。

---

## 执行流程

### 单次调用（总分钟数 ≤ 9）

用 `timeout` 参数等于"总分钟数 × 60"调 bash，timeout 设为 `540000`（9 min，留 1 分钟余量）。

```
Bash: bash .claude/skills/pr-watch/watch.sh <PR#> --timeout <分钟数*60> --interval <秒数>
     timeout: 540000
```

- Bash **foreground 运行**（不要 `run_in_background`），让会话被阻塞住——这就是"占用会话"的关键
- 观察退出码和 stdout

### 链式调用（总分钟数 > 9）

算一个截止时间戳，循环调脚本，每次最多 9 分钟，直到：
- 脚本 exit 0（有活动）→ 结束循环，报告用户
- 达到截止时间 → 结束循环，告诉用户超时
- 用户中断 → 停

**每次链式调用前**在用户面前简短说一句 "第 N 段开始，剩 X 分钟"，别完全沉默（否则像卡住）。

---

## 读取结果

脚本 stdout 格式：

```
[watch] owner/repo#42 — baseline 5 items, timeout 540s, interval 60s
[NEW] activity detected after 3 polls

=== review-comment ===
user: claude[bot]
at:   2026-04-21T22:45:12Z
file: agent/packages/tiandao/src/arbiter.ts:145
url:  https://github.com/.../pull/42#discussion_r...
---
<评论正文>

=== issue-comment ===
user: codex
at:   ...
---
<评论正文>
```

或超时：

```
[TIMEOUT] no new activity in 540s (9 polls)
```

### 给用户的报告格式

**有活动时**：
1. 一句话总结（谁 · 什么类型 · 核心诉求）
2. 按 reviewer 分组列原文要点（不要整段贴，挑关键）
3. 如果是行内评论，带上 `file:line`
4. 问一句"要不要我处理这条？"——不要擅自改代码

**超时时**：简短一行 "X 分钟内 PR #N 无新活动"，问是否续盯。

---

## 不要做的事

- **不要用 run_in_background**。这个 skill 的目的就是阻塞会话，让用户放心离开；后台跑的话用户还得 polling 我，反而费 token。
- **不要自己改代码响应评论**。只报告。用户看完再决定下一步（或 @claude 在 PR 里叫 mention workflow 处理）。
- **不要超过 9 分钟单次 bash**。Claude Code Bash 工具硬超时 10 分钟，到了会 kill 掉脚本丢失状态。
- **不要在 baseline 阶段报告已有评论**。skill 只关心启动后的"新"活动，老评论不 count。
- **不要静默链式**。每段之间给用户一个短反馈，否则看起来像死了。

---

## 使用场景

- PR 刚交给 Codex 或 Claude 审，想等评审意见回来立刻看到
- 自己给人发了 PR 想等对方 review，离开工位前挂一个
- 多个 PR 并行推进时，指定盯最关键那个

---

## 示例

用户：「盯一下 PR 42 的评论，20 分钟超时」
→ `PR=42`, 总分钟=20, 轮询秒=60（默认）
→ 因为 > 9，链式：先跑 9 分钟，超时再跑 9 分钟，再跑 2 分钟

用户：「/pr-watch 35」
→ `PR=35`, 默认 9 分钟，60 秒轮询，单次调用

用户：「看看 47 有没有新评论，每 30 秒查一次，最多 5 分钟」
→ `bash watch.sh 47 --timeout 300 --interval 30`，单次调用
