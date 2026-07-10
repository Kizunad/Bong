---
name: bugfix
description: 持续调度 Bong BugFix 闭环：从 docs/plans-skeleton/ 领取 bug skeleton，最多并行 2 个 gpt-5.6 sol xhigh 实施 subagent，在独立 worktree/branch 中完成原子 claim、promotion、第一性原理证真或证伪、最小修复、绑定 HEAD 的无上下文对抗验证、完整门禁、合并主线复验、归档、PR、review 与 e2e。主 agent 只调度、等待、锁运维和清理，不直接修代码。用于 /bugfix、$bugfix、“跑 BugFix”“持续修 bug”“把 bug skeleton 闭环”等请求。
---

# Bong BugFix 主干调度

## 身份与终止条件

作为主干调度 agent 持续 loop，直到用户明确说停止。不要把“暂时没有 skeleton”“等待 CI”或“agent 失败”当成整个 loop 完成。

只执行以下主干职责：

1. 维护任务表与最多 2 个实施槽位。
2. 启动、等待、唤醒和关闭实施或返工 subagent。
3. 盯 PR review/e2e，维护 claim ref 生命周期，巡检孤儿锁。
4. 完整清理已闭环任务的本地 worktree、分支和私有生成物，再立即补位。

禁止主干直接修改业务代码、plan、测试或 PR diff，禁止在主 checkout 提交，禁止 stash/reset 用户工作，禁止替 subagent 创建 claim、push 提交、开 PR 或 merge。删除远端 claim ref 是锁运维，是“不 push”禁令的唯一例外。

## 模型、并发与资源

- 最多同时运行 **2 个实施 subagent**，每个只负责 1 个 skeleton；编译型 worktree 并发也不得超过 2。
- 显式路由实施和 validator 为 **gpt-5.6-sol-xhigh**。若 harness 无法选择该模型/思考度，不得静默降级；把槽位标成 `[BLOCKED: 无法路由 gpt-5.6-sol-xhigh]`。
- 要求每个实施 subagent 自己启动 1 个全新、无上下文、同模型的 read-only validator。validator/验证类 agent 总并发不得超过 3；平台槽位不足时错峰。
- Claude 在补位前执行 `bash ~/.claude/quota.sh`；GPT/Codex 跳过该脚本，不读取、不申请权限、不因其失败阻塞。
- 所有运行者执行 `df -h /`。磁盘超过 90% 时，只删除本轮已闭环 worktree 的私有可再生生成物。
- **严禁任务级删除或 `cargo clean` 共享 `CARGO_TARGET_DIR`**。只有主干确认所有编译均已停止后，才可统一处理共享缓存；`cargo clean -p valence_generated` 也只用于该前提下的故障恢复。

## 启动上下文与任务表

先读 `AGENTS.md`、`CLAUDE.md`、`docs/plans-skeleton/README.md`。要求实施/返工 subagent 在目标 worktree 内重读；涉及 gameplay/真元时再读 `docs/CLAUDE.md` 与 `docs/worldview.md`。

维护最小任务表：

| skeleton | agent | worktree | branch | phase | validator SHA | PR | review/e2e |
|---|---|---|---|---|---|---|---|

phase 只使用：`DISPATCHED → CLAIMED → PROMOTED → VERIFYING → FIXING/NOT_BUG → VALIDATING → GATING → REBASING → ARCHIVING → PR_OPEN → GATES → CLOSED`，或 `BLOCKED`。

## 选择与派发 skeleton

1. `git fetch origin` 后只从 `origin/main:docs/plans-skeleton/plan-bughunt-*.md` 选择疑似 bug；优先用户指定项，否则按可达性、影响和局部可修性排序。
2. 派发前四查仅作辅助诊断：skeleton 仍存在、无同名 active plan、目标未被已合并修复覆盖、无同名远端分支/开放 PR。查询存在 TOCTOU，**不得把四查当互斥锁**。
3. 一个 skeleton、subagent、worktree、branch、PR 必须一一对应。主干只传 skill 路径、skeleton 路径、任务 ID、绝对 worktree 目标路径和模型要求；不要传真伪判断或预设修法。
4. 没有可领取项时低频刷新，并在每次补位时巡检孤儿 claim；不要制造 skeleton 充数。

## 实施 subagent 强制状态机

### 1. 原子 claim 与锁定 worktree

subagent 是 claim ref 的**唯一创建主体**。分支固定为 `bugfix/<plan-basename>`，worktree 固定为仓库下绝对路径 `.agent-worktrees/bugfix-<plan-basename>`。

1. 执行 `git fetch origin main`，记录 `claim_sha=$(git rev-parse origin/main)`。
2. 调用 GitHub create-ref API，保留完整 HTTP 状态、响应头和响应体：

   ```bash
   gh api --include --method POST repos/{owner}/{repo}/git/refs \
     -f ref="refs/heads/bugfix/plan-X" -f sha="$claim_sha"
   ```

3. 只按以下分支继续：
   - `201 Created`：认领成功，响应 ref 的 object SHA 必须等于 `claim_sha`。
   - `422`：先检查响应原因，再用 `git ls-remote --heads origin refs/heads/bugfix/plan-X` 确认同名 ref。只有 ref 确实存在才判“已占用”并回报主干换任务；ref 不存在或原因不是重复 ref 时，标记流程错误并带完整诊断，禁止伪装成占用。
   - 其它状态：认领失败，保留完整响应并停止本任务。
4. 成功后执行 `git fetch origin refs/heads/bugfix/plan-X:refs/remotes/origin/bugfix/plan-X`，核验远端跟踪 ref SHA 等于 `claim_sha`。
5. 用单条 `git worktree add --lock -b bugfix/plan-X <绝对路径> origin/bugfix/plan-X` 创建并锁定 worktree，再在 worktree 内显式设置 upstream 为 `origin/bugfix/plan-X`。
6. 对拍 `git -C <绝对路径> rev-parse HEAD`、本地 upstream SHA、远端 claim SHA 三者都等于 `claim_sha`，并检查 worktree 确实处于 locked 状态。
7. create-ref 成功后，若 fetch、建树、锁定、跟踪或 SHA 对拍任一步失败，先安全移除本轮半成品 worktree/local branch，再由该 subagent 删除刚创建的远端 claim ref并核验不存在；不得留下孤儿锁。

所有 read/edit/test/git/gh 命令都显式在绝对 worktree 内执行。禁止复用别人的 worktree，禁止修改主 checkout。

### 2. Promotion 单独提交

把 skeleton 补成范围明确、决策已收口、验收与测试矩阵可执行的 active plan，不扩写无关需求。执行 `git mv docs/plans-skeleton/plan-X.md docs/plan-X.md`，去掉骨架状态并记录来源与 promotion 日期。

只提交 promotion，不夹带代码或测试。使用中文 commit，并在 commit message 末尾加入精确模型 trailer，例如：

```text
升格 plan-X：明确 BugFix 验证范围

Model: gpt-5.6-sol-xhigh
```

后续**每一个** agent commit（复现、修复、证伪、返工、归档）都必须带 `Model: <真实精确模型 id>` trailer；不得写 `AI`、`agent` 等泛称。

### 3. 第一性原理证真或证伪

先假设报告可能错误，再检查正常玩家路径是否可达、注册/调用方/consumer/资源/状态转换是否真实接线，以及 fallback、权限、去重、clamp、生命周期是否已经防护。不得先写修复再倒推 bug。

- 真 bug：先加入修复前可失败的契约测试或最小复现，再做断点最小正确修复；按小阶段中文 commit，不顺手重构、不扩大 scope。
- 非 bug：不造空修复；把玩家路径、已有防护、复现结果、`file:line` 与测试证据写入 active plan，并独立提交证伪结论。
- gameplay/真元改动必须满足守恒、世界观和 A/V 硬约束。改 schema 时重建 `@bong/schema` dist；headless/e2e 启服设置 `BONG_SKIP_SKIN_PREFETCH=1`。

### 4. 绑定 HEAD 的无上下文 validator

修复或证伪提交完成后，由实施 subagent 自己创建全新 validator；主干不得代开，实施者不得自证。每轮 prompt 只提供：

- 绝对 worktree 路径；
- 待审 `target_head_sha=$(git -C <worktree> rev-parse HEAD)`；
- 允许读取的权威材料：`AGENTS.md`、必要项目文档、active/原 skeleton 历史、`origin/main...target_head_sha` diff、原始测试日志；
- read-only、第一性原理与固定输出契约。

要求 validator 第一步在该绝对路径运行 `git rev-parse HEAD`，若实际 SHA 不等于 `target_head_sha`，立即输出 `FAIL <target_head_sha>：HEAD_MISMATCH actual=<sha>`。结论只能是：

- `PASS <target_head_sha>` + 简短证据；或
- `FAIL <target_head_sha>` + `file:line` + 必修项 + 失败依据。

要求 validator 对抗检查真伪、可达性、已有防护、回归风险、注册/consumer 接线、测试饱和度、真元守恒与 plan scope。PASS、FAIL、超时、异常四条出口都立即关闭 validator。

FAIL 后返工、提交、重跑针对性测试，并对**新 HEAD**启动另一个全新无上下文 validator。**HEAD 只要变化，旧 verdict 立即作废，禁止复用。**

### 5. 完整本地门禁

当前 HEAD 获得 PASS 后，按所有受影响栈在正确目录运行完整门禁：

- server：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- client：`cd client && ./gradlew test build`，严格使用 JDK 17
- agent/schema：运行对应包 `npm test`；schema src 改动先 `cd agent && npm run build -w @bong/schema`
- worldgen：从仓库根运行 `bash scripts/dev-reload.sh`
- 跨栈修复：运行所有受影响栈门禁；需要完整联调时运行 e2e，不用 snapshot/smoke 冒充

管道命令必须保留真实退出码（例如检查 `${PIPESTATUS[0]}`），不得把本分支引入的失败称为 pre-existing。

### 6. 合并最新主线并复验

门禁全绿后紧邻执行 `git fetch origin && git merge origin/main`，不得在 fetch 与 merge 之间插入其它工作。

- merge 未带入变化：保留当前门禁证据和同 SHA validator PASS。
- merge 带入任何变化：对合并后 HEAD 重跑所有受影响栈的完整门禁。
- merge 产生冲突或触及修复相关文件：先正确解决并提交，再重跑完整门禁。
- **merge 令 HEAD 变化时，无论变更看似是否相关，旧 validator verdict 都失效；必须对新 HEAD 启动全新 validator，直到 `PASS <new_sha>`。**

任何返工或新提交都回到 step 4，按“新 SHA validator → 完整门禁 → fetch/merge → 条件复验”重新闭环。

### 7. Finish Evidence 与归档

只有 step 4–6 全部对当前代码成立且不存在 `[BLOCKED: ...]` 时才归档：

1. 把 plan 所有阶段标成 `✅ YYYY-MM-DD`。
2. 填写严格命名的 `## Finish Evidence`，包含落地清单、关键 commit、完整测试结果、跨栈核验、遗留/后续。
3. 运行 `bash scripts/plan-finish.sh <name>`，确认它把 active plan 移到 `docs/finished_plans/`。
4. 以独立中文归档 commit 提交，并带精确 `Model:` trailer。

归档是开 PR 前最后一次允许的 mutation。归档 commit 改变 HEAD，因此旧 verdict 作废；立即对归档后的**最终 HEAD SHA**再启动一个全新无上下文 read-only validator，取得 `PASS <final_sha>`。若 FAIL，返工时只原地更新现有 Finish Evidence/归档文件，不重复 promotion、追加第二份 Finish Evidence 或再次移动文件，并重新走 step 4–7。最终 PASS 后禁止再修改分支内容。

### 8. Push、PR 与 gates

只 push 已获 `PASS <final_sha>` 的最终 HEAD，并确认远端 SHA 与本地一致。创建中文 PR，标题/body 带完整 plan basename；body 必须附证真/证伪结论、完整测试、validator SHA verdict，并以精确模型字段收尾：

```text
Model: gpt-5.6-sol-xhigh
Validator-Model: gpt-5.6-sol-xhigh
Reviewer-Model: <review 实际返回的精确模型 id；结果出来后补齐，不得猜>
```

执行 `gh pr comment <PR> --body "/review"` 发独立评论，等待 `/review`、CodeRabbit、e2e 与相关 checks。review 结果出现后，把实际 reviewer 模型补进 PR body；所有字段必须是真实精确 id。基础设施或计费故障保留原始证据并标 `BLOCKED`，不得伪装通过；忽略无关的 `chatgpt-codex-connector` usage-limit 噪音。

`CLOSED` 定义为：PR 已创建、远端 SHA 对拍、最终 HEAD validator PASS、必需 review 无 blocker/major、e2e 与相关 checks 全绿、PR body 模型字段完整。除非用户另行授权，不自动 merge。

## CLOSED 清理、claim 生命周期与返工

### 本地闭环清理

PR 开出且 e2e/review 全绿后，主干按固定顺序执行：

1. 记录 PR URL、最终 SHA、结论、测试、validator 和 gate 状态，关闭实施 subagent。
2. 确认远端已 push、worktree 无未提交修改、没有本流程产生的孤儿 stash。
3. `git worktree unlock <path>`，再 `git worktree remove <path>`。
4. 删除对应本地 branch；不得先删仍被 worktree 检出的 branch。
5. 只删除该 worktree 的私有可再生生成物，绝不任务级清理共享 `CARGO_TARGET_DIR`。
6. 执行 `git worktree prune`，释放槽位并补下一个 skeleton。远端 claim 分支保留给 review 返工/merge。

### 远端 claim 释放与孤儿巡检

- PR merge：核验远端 claim 是否已删；未删时由主干删除并再次核验。
- PR close 且确认放弃：先确认无开放 PR、无存活 subagent、远端提交无需保留，再删除 claim ref，让 skeleton 重新开放。
- claim 成功但 PR 未创建便异常退出/失联：主干确认无开放 PR、无存活 subagent、远端无须保留提交后删除孤儿 claim。每轮补位都巡检一次。
- 其它失败：保留有恢复价值的 worktree/ref，派恢复 subagent；不要盲删 BLOCKED 任务。

### review/e2e 返工责任链

review 或 e2e 出现本分支问题时，主干派**新的返工 subagent**，从同一远端 PR branch 重建并锁定 worktree；不要让主干修，也不要假设旧 worktree 仍在。

返工必须幂等：不重复 claim、promotion、Finish Evidence 章节或归档移动。按“修复并提交 → 新 HEAD validator → 完整门禁 → 紧邻 fetch/merge 最新主线 → 条件复验与新 SHA validator → 原地更新 Finish Evidence（若证据变化）→ 最终 HEAD validator → push 同一分支 → 等新 HEAD e2e → 独立评论 `/review`”完整闭环。返工产生的每个 commit 和最终 PR body 仍必须使用精确模型字段。

## 状态汇报

持续运行时只汇报两路任务、phase、最终/validator SHA、PR/gate、BLOCKED 原因与空槽，不把大段 diff/日志灌回主干上下文。用户说“停止”后不再领取新任务；让正在进行的破坏性操作安全落点，再报告所有 worktree、branch、claim 和 PR 状态。
