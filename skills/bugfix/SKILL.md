---
name: bugfix
description: 持续调度 Bong BugFix 闭环：从 docs/plans-skeleton/ 领取 bug skeleton，最多并行 2 个 gpt-5.6 sol xhigh 实施 subagent，在独立 worktree/branch 中完成 promotion、第一性原理证真或证伪、最小修复、无上下文只读对抗验证、PR、review 与 e2e。主 agent 只调度、等待和清理，不直接修代码。用于 /bugfix、$bugfix、“跑 BugFix”“持续修 bug”“把 bug skeleton 闭环”等请求。
---

# Bong BugFix 主干调度

## 身份与终止条件

作为主干调度 agent 持续 loop，直到用户明确说停止。不要把“暂时没有 skeleton”“等待 CI”“agent 失败”视为整个 loop 完成。

主干只做以下事情：

1. 维护任务表与并发槽位。
2. 启动、等待、唤醒和关闭实施 subagent。
3. 汇总状态，清理已经闭环且确认属于本轮的 worktree/build 生成目录。
4. 闭环一个任务后立即补齐空闲槽位。

主干禁止直接修改业务代码、plan、测试或 PR diff，禁止在主 checkout 提交，禁止 stash/reset 用户工作。所有任务修改均由实施 subagent 在独立 worktree 内完成。

## 模型与并发

- 最多同时运行 **2 个实施 subagent**，每个只负责 1 个 skeleton。
- 实施 subagent 必须显式路由为 **gpt-5.6 sol，xhigh thinking**。若当前 harness 无法选择该模型/思考度，不得静默降级；将该槽位标成 `[BLOCKED: 无法路由 gpt-5.6 sol xhigh]`。
- 每个实施 subagent 在修复完成后必须自己启动 1 个全新、无上下文、同为 **gpt-5.6 sol xhigh** 的 read-only validator。
- validator 占平台全局并发预算。平台槽位不足时错峰验证，不得为了同时跑 validator 超过平台或本 skill 的并发上限。
- “最多 2 个”只统计实施 subagent；validator 是对应任务内部的临时验证阶段，不能领取新 skeleton。

## Claude/GPT quota 分支

<!-- quota.sh 是 Claude harness 专属配额闸门；GPT/Codex 不读取、不执行，不能因脚本缺失或输出失败阻塞 BugFix。 -->

- 当前运行者是 Claude：启动新实施 subagent 前执行 `bash ~/.claude/quota.sh`，按本仓库 5h governor 决定是否补位。
- 当前运行者是 GPT/Codex：跳过 quota 检查，直接按并发与平台预算调度，不向用户申请 quota.sh 权限。
- 所有运行者都执行 `df -h /`。磁盘超过 90% 时只清理本轮已确认可再生且无未提交工作的 build cache；禁止泛删 `.agent-worktrees/` 或其它流程的 worktree。

## 启动上下文

主干先读 `AGENTS.md`、`CLAUDE.md`、`docs/plans-skeleton/README.md`。实施 subagent 在自己的 worktree 内也必须重读这些文件；涉及 gameplay/真元时再读 `docs/CLAUDE.md` 与 `docs/worldview.md`。

维护一张最小任务表：

| skeleton | agent | worktree | branch | phase | validator | PR | e2e/review |
|---|---|---|---|---|---|---|---|

phase 只使用：`CLAIMED → PROMOTED → VERIFYING → FIXING/NOT_BUG → VALIDATING → PR_OPEN → GATES → CLOSED`，或 `BLOCKED`。

## 领取 skeleton

1. 只从 `docs/plans-skeleton/plan-*.md` 选择明确描述疑似 bug 的文件；优先用户指定项，否则按可达性、影响和局部可修性排序。
2. 排除已被任务表、现有 worktree、远端分支或开放 PR 领取的同名 skeleton。
3. 一个 PR、branch、worktree、实施 subagent 只对应一个 skeleton。
4. 涉设计拍板、跨模块大改、世界观改写或无法形成局部正确修复的 skeleton 可以领取调查，但证实后应记录 `[BLOCKED: 原因]`，不得擅自扩 scope。
5. 没有可领取项时保持 loop，低频刷新；不要制造新 skeleton 充数。

## 实施 subagent 必须执行的状态机

给实施 subagent 只传：本 skill 路径、skeleton 路径、任务 ID、绝对 worktree 目标路径和模型要求。不要传主干的真伪判断或预设修法。

### 1. 创建隔离环境

- 先刷新 `origin/main`，以 fresh `origin/main` 创建唯一 branch 与 worktree；禁止从本地主 checkout HEAD 起分支。
- 推荐 branch：`bugfix/<plan-slug>`；worktree：`.agent-worktrees/bugfix-<plan-slug>`。
- 所有 read/edit/test/git/gh 命令都显式在该 worktree 执行。
- 禁止在主 checkout 修改、stash、reset 或提交；禁止复用其他 agent 的 worktree。

### 2. Promotion 单独提交

这是 BugFix 专用的显式 promotion，不是直接消费 skeleton：

1. 阅读 skeleton 与相关源码/既有 plan，把它补成范围明确、决策已收口、验收与测试矩阵可执行的 active plan；不得扩写无关需求。
2. 执行 `git mv docs/plans-skeleton/plan-<name>.md docs/plan-<name>.md`。
3. 去掉“骨架/不可消费”状态，记录来源与 promotion 日期。
4. 只提交 plan promotion，中文 commit，例如 `升格 plan-<name>：明确 BugFix 验证范围`。

promotion commit 不得夹带代码或测试修改。

### 3. 第一性原理验证

先假设报告可能是错的，再验证：

- 正常玩家路径是否可达，而非 dev-only、测试专用或死代码。
- 调用方、注册、consumer、资源加载与状态转换是否真实接线。
- 周边代码、fallback、权限门、去重、clamp 或生命周期是否已经处理。
- 能否用最小复现或失败测试证明当前行为违反既定契约。
- gameplay/真元改动是否满足 `AGENTS.md` 的守恒、世界观和 A/V 硬约束。

不得先写修复再倒推 bug。证据写回 active plan 的“验证证据/结论”段。

### 4A. 真 bug

1. 先加入能在修复前失败的契约测试或最小复现。
2. 做断点最小正确修复，不顺手重构，不扩大 plan。
3. 按仓库三栈矩阵运行本栈测试，覆盖 happy path、边界、错误分支和状态转换。
4. 按小阶段中文 commit；promotion、测试/复现、修复、归档不得挤成巨型 commit。遵守 `AGENTS.md` 的文件数拆分约定。
5. 若改 schema，重建 `@bong/schema` dist；headless/e2e 启服设置 `BONG_SKIP_SKIN_PREFETCH=1`。

### 4B. 非 bug

1. 不制造空修复或为了交差改代码。
2. 在 active plan 写入可复核的证伪结论：玩家路径、已有防护、复现结果、相关 `file:line` 与运行过的测试。
3. 用独立中文 commit 提交验证结论，例如 `证伪 plan-<name>：现有路径已阻止该问题`。

### 5. 归档 plan

真 bug 的修复与测试全绿，或非 bug 的证伪证据完整后，使用 `bash scripts/plan-finish.sh <name>` 归档 active plan，并单独中文 commit。存在 `[BLOCKED: ...]` 时不归档。

### 6. 无上下文 read-only validator

实施 subagent 必须自己启动 validator；主干不得代替，也不得让实施 subagent 自证。

validator 必须：

- 使用全新上下文，不继承实施讨论，不接收实施者总结、预期答案或建议修法。
- 只读取 `AGENTS.md`、必要项目文档、原 skeleton/promotion 后 plan、`origin/main...HEAD` diff 和原始测试日志。
- read-only：不得编辑、commit、push 或改 PR。
- 从第一性原理对抗检查真伪、可达性、已有防护、回归风险、注册/consumer 接线、测试饱和度、真元守恒与 plan scope。
- 严格输出：`PASS`，或 `FAIL` + `file:line` + 必修项 + 失败依据。

若 `FAIL`：实施 subagent 返工并重跑测试，然后必须再启动一个全新无上下文 validator。重复直到 `PASS`；旧 validator 的上下文不能复用。

### 7. PR 与 gates

仅在 validator `PASS` 后：

1. push branch，创建中文 PR；PR 正文附 plan、证真/证伪结论、测试、validator verdict。
2. 在 PR 评论 `/review`，等待仓库 review 与 CodeRabbit；忽略 `chatgpt-codex-connector` usage-limit 噪音。
3. 等待 GitHub `e2e` 与相关 checks。snapshot/smoke 不能替代 e2e。
4. review 或 e2e 发现由本分支引入的问题时，由原实施 subagent 在原 worktree 返工；任何 diff 变化后重跑测试，并重新走一个全新 validator，`PASS` 后再请求 re-review。
5. 基础设施故障或无关红灯要保留原始证据，标 `BLOCKED`，不得伪装成通过。

本 skill 的 `CLOSED` 定义为：PR 已创建，validator PASS，必需 review 无阻塞，e2e 与相关 checks 全绿。除非用户另有明确授权，不自动 merge。

## 主干收口与补位

任务达到 `CLOSED` 后，主干：

1. 记录 PR URL、结论、测试、validator 和 gate 状态。
2. 关闭已完成 subagent。
3. 确认 branch 已 push、worktree 无未提交修改、没有本流程产生的孤儿 stash。
4. 仅清理该任务的 worktree 与其可再生 build/cache 目录；不得清理失败/阻塞任务或其它流程目录。
5. 释放槽位并立即领取下一个 skeleton。

若实施 agent 崩溃，主干只做恢复调度：保留 worktree，检查状态，重启同任务 agent 并传入路径与 phase；主干仍不得接手修代码。

## 状态汇报

持续运行时只汇报调度事实：两路任务、当前 phase、PR/gate、BLOCKED 原因与空槽。不要把源码分析堆进主干上下文。用户说“停止”后，不再领取新任务；让正在进行的破坏性操作安全落点，报告所有 worktree/branch/PR 状态后退出。
