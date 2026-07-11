---
name: bugfix
description: 持续调度 Bong BugFix 闭环：按用户启动参数并行实施 subagent，在独立 worktree/branch 中完成原子 claim、promotion、第一性原理证真或证伪、最小修复、绑定干净 HEAD 的无上下文对抗验证、完整门禁、合并主线复验、归档、PR、review 与 e2e。用户未指定时默认 2 路实施、gpt-5.6-sol-xhigh 实施与 validator。主 agent 只调度、等待、锁运维和清理，不直接修代码。用于 /bugfix、$bugfix、“跑 BugFix”“持续修 bug”“把 bug skeleton 闭环”等请求。
---

# Bong BugFix 主干调度

## 身份与终止条件

作为主干调度 agent 持续 loop，直到用户明确说停止。不要把“暂时没有 skeleton”“等待 CI”或“agent 失败”当成整个 loop 完成。

只执行以下主干职责：

1. 维护任务表与用户指定的实施槽位。
2. 启动、等待、唤醒和关闭实施或返工 subagent。
3. 盯 PR review/e2e，维护 claim ref 生命周期，巡检孤儿锁。
4. 完整清理已闭环任务的本地 worktree、分支和私有生成物，再立即补位。

禁止主干直接修改业务代码、plan、测试或 PR diff，禁止在主 checkout 提交，禁止 stash/reset 用户工作，禁止替 subagent 创建 claim、push 提交、开 PR 或 merge。删除远端 claim ref 是锁运维，是“不 push”禁令的唯一例外。

## 启动参数、并发与资源

- 启动时读取并保持三个独立参数：实施数量 `N`、实施模型、validator 模型。用户未指定时才默认 `N=2`、实施模型 `gpt-5.6-sol-xhigh`、validator 模型 `gpt-5.6-sol-xhigh`；不得用默认值覆盖用户输入。
- 每个实施 subagent 只负责 1 个 skeleton。实施总槽位按 `N`，但**同时执行编译的 worktree 始终不得超过 2**；非编译调查槽位不因这个资源上限被错误固定为两路。
- 要求每个实施 subagent 自己启动 1 个全新、无上下文、使用 validator 参数模型的 read-only validator；validator/验证类 agent 总并发始终不得超过 3，平台槽位不足时错峰。
- 把平台总 agent 槽位纳入启动准入：主干占 1 槽，并永久为实际 validator 预留至少 1 槽。容量可查时，每次补位按实时快照计算 `min(N, 2, max(0, platform_total - live_agents - 未入 snapshot reservation - validator_reserve - 未计入 live 的主干占位))`；容量不可查时保守按 2 路实施。平台满载、已有无关 agent 或 outstanding reservation 都会减少可启动数；超额任务进 FIFO，不 spawn。
- validator 授权必须同时满足逻辑 `validator_token ≤3` 和平台真实剩余槽位；主干通过当前 harness 实际提供的状态查询对拍 live agent。每个已 GRANTED/ACKED/RECOVERING、但尚未出现在 live snapshot 的 validator token 都先预占 1 个平台槽；只有状态查询确认对应 validator 已出现后才转为 snapshot-accounted，避免重复授权或双扣。容量不可查时 validator 一次只授权 1 个。
- 用户指定模型不可用时，先按用户明确允许的候选路由；没有允许的替代模型时请求用户决策。只有默认模型不可用且用户未指定时，才说明可用模型并请求选择；不得静默降级，也不得直接阻断整个 loop。
- Claude 在补位前执行 `bash ~/.claude/quota.sh`；GPT/Codex 跳过该脚本，不读取、不申请权限、不因其失败阻塞。
- 所有运行者执行 `df -h /`。磁盘超过 90% 时，只删除本轮已闭环 worktree 的私有可再生生成物。
- **严禁任务级删除或 `cargo clean` 共享 `CARGO_TARGET_DIR`**。只有主干确认所有编译均已停止后，才可统一处理共享缓存；`cargo clean -p valence_generated` 也只用于该前提下的故障恢复。

### harness 能力探测与适配

启动时先枚举当前宿主真实可用工具，只选择一个适配分支；禁止把任一命名空间描述成通用 Codex API，也禁止调用当前分支未暴露的接口。

| harness | 启动/消息/等待/恢复/状态映射 |
|---|---|
| 有 `collaboration` namespace | `spawn_agent` / `send_message` / `wait_agent` / `followup_task` / `list_agents` |
| `multi_agent_v1` | `spawn_agent` / `send_input` / `wait_agent` / `resume_agent` / `close_agent`；状态只使用该运行时实际暴露的 agent status/list 返回，不虚构查询名 |
| Claude | `Agent` / `SendMessage` / `ScheduleWakeup`；agent 完成通知与 Agent 状态作为恢复依据 |

- 支持子 agent 主动发消息的 harness：用本分支真实 message API 发送结构化 token 消息；主干用本分支真实 wait/status API 接收和对拍。
- 不支持子 agent 在运行中等待父级输入，或能力探测不确定时：强制使用 **phase-yield/checkpoint**。实施 agent 结构化返回 `TOKEN_REQUEST + checkpoint` 后结束当前 turn，主干入 FIFO；获准后通过该 harness 的 `resume_agent` / `followup_task` / `send_input` / Agent continuation 恢复同一任务。不得假设子 agent 能主动 `wait_agent` 等父级。
- 能力不足以恢复同一任务时，持久化绝对 worktree、phase、HEAD、request/token、测试证据，启动同任务 continuation；不得传实施结论或丢失 checkpoint。

### 主干唯一 token 准入协议

主干是资源状态表、等待队列和授权消息的**唯一持有者**，串行处理申请，禁止 subagent 自行计数或先做后报：

| token | 容量 | 持有阶段 | 状态表字段 |
|---|---:|---|---|
| `compile_token` | 2 | 进入任何编译/完整门禁前至该轮命令结束 | token id、task、agent、grant time、phase |
| `validator_token` | 3 | spawn validator 前至 validator 已关闭 | token id、task、agent、validator、target SHA、grant time |

按上方适配器选择真实接口完成可恢复握手：

1. 实施 agent 发送或 phase-yield 返回：`TOKEN_REQUEST{"request_id":"<uuid>","type":"compile|validator","task":"<id>","agent":"<canonical agent id>","phase":"<phase>","head":"<sha>","checkpoint":"<resume data>"}`；**未收到有效 grant 禁止编译或 spawn validator**。
2. 主干按 `request_id` 幂等去重，落唯一状态表、排入 FIFO。grant 前重新读取权威 task 的 task/phase/head/generation；逻辑容量与实时平台槽位都满足时，先原子登记 holder，再返回 `TOKEN_GRANTED{"request_id":"...","token_id":"...","type":"...","task":"...","phase":"...","head":"...","generation":N,"expires_at":"..."}`。
3. 实施 agent 核对 request/task/phase/head/generation/expiry 后发送或返回 `TOKEN_ACK{...}`，ACK 后 token 才可使用。主干对**每次** ACK（包括重复 ACK）重新读取权威状态；任何漂移都原子 stale/cancel。重复 request 返回同一队列状态或同一 grant；状态未漂移时重复 ACK 幂等。
4. validator token ACK 后，实施 agent 用 harness spawn 返回的 **canonical validator agent ID** 上报 `VALIDATOR_SPAWNED{"request_id":"...","token_id":"...","validator_agent_id":"...","task":"...","phase":"...","target_sha":"...","generation":N}`。该 ID 与 request/token/task/phase/SHA/generation 一对一绑定；一个 validator 实例只能消费一个 reservation。只有 live snapshot 精确包含同一 canonical ID 才可标 snapshot-accounted；无关 agent、另一 validator 或仅总数增长均无效。
5. 已结束/idle agent 通过当前 harness 的 resume/followup 入口恢复，payload 必须包含 `checkpoint + request_id + token_id + phase + head + generation`；恢复成功前状态为 `RECOVERING`，恢复边界再次对拍权威状态。禁止另开无状态 agent 猜测续点。
6. 完成、FAIL、超时、异常时，实施 agent 发送或返回 `TOKEN_RELEASED{...}`；排队取消或 grant 失效返回 `TOKEN_CANCELLED{...}`。主干确认后释放并回 `TOKEN_RELEASE_ACK{...}`。
7. grant 绑定 request_id、task、agent、phase、head、generation。任一变化、token 已回收、非 FIFO 当前授权或 ACK 前过期都原子 stale；旧 token 禁止 ACK/recovery/release。
8. 重复 RELEASE/CANCEL 必须幂等 no-op；同 request_id 但 payload 不同视为协议错误。每次唤醒用当前 harness 实际状态查询对拍 holder；失联先进入 `RECOVERING`，只有 resume/send-input/followup 明确失败或恢复 TTL 到期才回收。回收后的迟到消息一律拒绝。
9. `compile_token` 在该轮门禁任一出口释放；`validator_token` 在 validator 关闭后任一出口释放，spawn 失败也释放。任务 BLOCKED/CLOSED、用户停止或 worktree 清理前，主干取消排队项并核验无悬挂 token。

从仓库根运行 `python3 skills/bugfix/scripts/state_machine_dry_run.py` 验证这套状态机。该 dry-run 不调用 GitHub 写 API，覆盖容量/FIFO、消息握手、异常回收、claim/main-sync/verdict 与持续等待契约；修改资源协议时必须同步更新并运行。

## 启动上下文与任务表

在任何 skeleton 四查、claim 或 promotion 之前，主干与实施/返工 subagent 都必须无条件完整读取 `docs/CLAUDE.md`；根 `AGENTS.md`、`CLAUDE.md` 与 `docs/plans-skeleton/README.md` 同时读取，但不能替代 `docs/CLAUDE.md`。worktree 建立后在任务面再次读取这些文件；涉及 gameplay/真元时额外读取 `docs/worldview.md`。

维护唯一任务/资源状态表；以下字段不得省略：

| skeleton | agent | worktree | branch | phase | generation | token_type | request_id | token_id | queue_pos | holder | request_phase | requested_at | granted_at | expires_at | ack | validator_agent_id | recovery_deadline | release_reason | checkpoint | head | validator evidence | gate evidence | PR | review/e2e |
|---|---|---|---|---|---:|---|---|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

phase 只使用：`DISPATCHED → CLAIMED → PROMOTED → VERIFYING → FIXING/NOT_BUG → FIX_VALIDATING → GATING → REBASING → REBASE_VALIDATING → ARCHIVING → FINAL_VALIDATING → PR_OPEN → GATES → CLOSED`，失联暂态 `RECOVERING`，或终态 `BLOCKED`。只有 `FINAL_VALIDATING → PR_OPEN`；合法边、必经里程碑、互斥分支、终态和返工回流以 dry-run 的 `TaskPhase/ALLOWED_EDGES` 为可执行契约。

状态迁移不能靠“走过 phase”解锁：三个 validating 关口都必须提交 `validator_agent_id + phase + target_sha + PASS + completed_at`，且 target SHA 等于权威当前 HEAD；`GATING → REBASING` 必须有当前 HEAD 的成功 gate evidence；主线同步、HEAD 变化或返工会递增 generation，并清除旧 SHA 的 validator/gate 证据与闭环里程碑。FAIL、缺证据、错误 SHA 或旧 generation 均不得推进。

## 选择与派发 skeleton

1. `git fetch origin` 后只从 `origin/main:docs/plans-skeleton/plan-bughunt-*.md` 选择疑似 bug；优先用户指定项，否则按可达性、影响和局部可修性排序。
2. 派发前四查仅作辅助诊断：skeleton 仍存在、无同名 active plan、目标未被已合并修复覆盖、无同名远端分支/开放 PR。查询存在 TOCTOU，**不得把四查当互斥锁**。
3. 一个 skeleton、subagent、worktree、branch、PR 必须一一对应。主干只传 skill 路径、skeleton 路径、任务 ID、绝对 worktree 目标路径和模型要求；不要传真伪判断或预设修法。
4. 没有可领取项时进入等待；不要制造 skeleton 充数。

## 等待协议

- 等待新 skeleton、资源槽、review 或 e2e 时采用约 1200 秒节奏：Claude 使用 `ScheduleWakeup`；Codex 使用产品 wait/monitor 或 goal continuation；其它 harness 使用其真实等待/续跑入口。不要声称未暴露的 harness 具有 `ScheduleWakeup`。
- 同一等待对象连续 3 轮无进展时升级告警、记录证据并继续对应 harness 的等待机制；不得因此结束 loop。只有用户明确停止，或出现确定性且必须由用户选择的决策点，才暂停等待输入。
- 禁止 shell `sleep` loop、短周期 GitHub 查询和持续占用 agent 的 busy-poll。

## 实施 subagent 强制状态机

### 1. 原子 claim 与锁定 worktree

subagent 是 claim ref 的**唯一创建主体**。分支固定为 `bugfix/<plan-basename>`，worktree 固定为仓库下绝对路径 `.agent-worktrees/bugfix-<plan-basename>`。

严格区分作用域：

- **控制面**：以下 claim、fetch、`git worktree add`、失败回滚、最终 remove/prune 命令必须在已经存在的主仓库绝对路径或专用调度目录执行，目标 worktree 尚未创建时不得把 cwd 指向它。
- **任务面**：只有 worktree 已创建、locked、upstream 与三方 SHA 对拍完成后，才允许在目标绝对 worktree 中读取任务代码、编辑、测试、commit、push 和操作 PR。

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
7. create-ref 成功后，若 fetch、建树、锁定、跟踪或 SHA 对拍任一步失败，在控制面安全移除本轮半成品 worktree/local branch，再由该 subagent 删除刚创建的远端 claim ref并核验不存在；不得留下孤儿锁。

进入任务面后，所有任务 read/edit/test/git/gh 命令都显式在绝对 worktree 内执行。禁止复用别人的 worktree，禁止修改主 checkout。

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
- 在 `VERIFYING`、`FIXING` 或 `NOT_BUG` 阶段运行任何会编译/构建的复现或针对性测试前，同样必须申请 `compile_token`；针对性测试与完整门禁共享容量，但用途和结果分开记录。

### 4. 绑定 HEAD 的无上下文 validator

修复或证伪提交完成后进入 `FIX_VALIDATING`。先执行 `git status --porcelain=v1 --untracked-files=all` 并要求输出为空；确认 index、工作区和所有预期生成文件都已分类，需进入 PR 的改动已形成带 `Model:` trailer 的 commit。脏工作区禁止启动 validator。

实施 subagent 必须先申请并收到主干的 `validator_token` 明确授权，才能创建全新 validator；主干不得代开，实施者不得自证。每轮 prompt 只提供：

- 绝对 worktree 路径；
- 待审 `target_head_sha=$(git -C <worktree> rev-parse HEAD)`；
- 允许读取的权威材料：`AGENTS.md`、必要项目文档、active/原 skeleton 历史、`origin/main...target_head_sha` diff、原始测试日志；
- read-only、第一性原理与固定输出契约。

要求 validator 第一步在该绝对路径运行 `git status --porcelain=v1 --untracked-files=all` 和 `git rev-parse HEAD`。工作区非空或实际 SHA 不等于 `target_head_sha` 时，立即输出 `FAIL <target_head_sha>：DIRTY_WORKTREE/HEAD_MISMATCH`。结论只能是：

- `PASS <target_head_sha>` + 简短证据；或
- `FAIL <target_head_sha>` + `file:line` + 必修项 + 失败依据。

要求 validator 对抗检查真伪、可达性、已有防护、回归风险、注册/consumer 接线、测试饱和度、真元守恒与 plan scope。verdict 返回后，实施 subagent 再次核验 `git status --porcelain=v1 --untracked-files=all` 为空且 HEAD 仍等于 `target_head_sha`；否则 verdict 失效。PASS、FAIL、超时、异常四条出口都立即关闭 validator。

FAIL 后返工、提交、重跑针对性测试，并对**新 HEAD**启动另一个全新无上下文 validator。**HEAD 或工作区只要变化，旧 verdict 立即作废，禁止复用。**

### 5. 完整本地门禁

当前 HEAD 获得 PASS 后，实施 subagent 必须先申请并收到主干的 `compile_token` 明确授权，才能按所有受影响栈在正确目录运行完整门禁：

- server：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- client：`cd client && ./gradlew test build`，严格使用 JDK 17
- agent/schema：运行对应包 `npm test`；schema src 改动先 `cd agent && npm run build -w @bong/schema`
- worldgen：从仓库根运行 `bash scripts/dev-reload.sh`
- 跨栈修复：运行所有受影响栈门禁；需要完整联调时运行 e2e，不用 snapshot/smoke 冒充

管道命令必须保留真实退出码（例如检查 `${PIPESTATUS[0]}`），不得把本分支引入的失败称为 pre-existing。

门禁若产生 tracked/untracked 文件，先分类：PR 所需产物必须单独提交并对新 HEAD 回 step 4；纯私有且可再生的 ignored 产物可保留到闭环清理。任何未分类或未提交改动都使旧 PASS 失效。

### 6. 同步最新主线并复验

门禁全绿后执行 `git fetch origin`，立即用 merge-base 分类，不允许直接运行会默认生成提交的 `git merge origin/main`：

- **already-up-to-date**：`origin/main` 已是 HEAD 祖先，不改 HEAD，保留当前门禁证据。
- **fast-forward**：HEAD 是 `origin/main` 祖先，执行 `git merge --ff-only origin/main`。fast-forward 没有 agent 新建 commit，因此无需新增 trailer；HEAD 变化后重跑受影响栈完整门禁，再进入下方 `REBASE_VALIDATING`。
- **diverged**：执行 `git merge --no-commit --no-ff origin/main`，禁止自动 commit。解决冲突后，先持有 `compile_token` 对未提交的合并结果运行受影响栈完整门禁；PASS 后用中文消息和精确 trailer 显式提交，例如 `git commit -m "合并主线：复验 plan-X 修复" -m "Model: <实际实施模型精确 id>"`。随后确认工作区干净并进入下方 `REBASE_VALIDATING`。

任何同步带入变化都必须重跑受影响栈完整门禁；冲突或触及修复相关文件时扩大针对性复验。**HEAD 或工作区变化会使旧 validator verdict 失效。**

三种同步分支收口后都进入 `REBASE_VALIDATING`，对同步判定后的当前干净 HEAD 重新申请 validator token；PASS 后才能进入 `ARCHIVING`。不得从首次 `FIX_VALIDATING` 或 `ARCHIVING` 直接开 PR。

任何返工或新提交都回到 step 4，按“新 SHA validator → 完整门禁 → fetch 后分类同步主线 → 条件复验”重新闭环。

### 7. Finish Evidence 与归档

只有 step 4–6 全部对当前代码成立且不存在 `[BLOCKED: ...]` 时才归档：

1. 把 plan 所有阶段标成 `✅ YYYY-MM-DD`。
2. 填写严格命名的 `## Finish Evidence`，包含落地清单、关键 commit、完整测试结果、跨栈核验、遗留/后续。
3. 运行 `bash scripts/plan-finish.sh <name>`，确认它把 active plan 移到 `docs/finished_plans/`。
4. 以独立中文归档 commit 提交，并带精确 `Model:` trailer。

归档是开 PR 前最后一次允许的 mutation。归档 commit 后进入 `FINAL_VALIDATING`；立即对归档后的**最终 HEAD SHA**再启动一个全新无上下文 read-only validator，取得 `PASS <final_sha>`。只有该 phase 能转入 `PR_OPEN`。若 FAIL，返工时只原地更新现有 Finish Evidence/归档文件，不重复 promotion、追加第二份 Finish Evidence 或再次移动文件，并重新走 step 4–7。最终 PASS 后禁止再修改分支内容。

### 8. Push、PR 与 gates

push 前再次要求 `git status --porcelain=v1 --untracked-files=all` 为空、HEAD 等于最后一次 `PASS <final_sha>`，并确认所有预期改动均已提交。用 `git log origin/main..HEAD` 检查本分支新增的**每一个 commit**都存在且仅使用真实精确 id 的 `Model:` trailer；缺失、空值、`AI`、`agent`、`unknown` 等值一律阻止 push。

只 push 该最终 HEAD，并确认远端 SHA 与本地一致。创建中文 PR，标题/body 带完整 plan basename；body 必须附证真/证伪结论、完整测试、validator SHA verdict。创建时只写入当时已知的两个精确模型字段：

```text
Model: <实际实施模型精确 id>
Validator-Model: <实际 validator 模型精确 id>
```

首轮 review 返回前，PR body 必须**省略** `Reviewer-Model`，禁止猜测、`pending` 或 `unknown` 占位。执行 `gh pr comment <PR> --body "/review"` 发独立评论；review 返回并能从权威结果确认精确模型后，使用 `gh pr edit` 原地补入 `Reviewer-Model: <精确 id>`。若 CLI 的项目字段查询故障，允许用等价 GitHub REST PATCH 更新同一 PR body。等待 `/review`、CodeRabbit、e2e 与相关 checks。

闭环前重新读取 PR body，严格校验 `Model`、`Validator-Model`、`Reviewer-Model` 三字段均存在且为实际精确 id；review 尚未返回模型时不得 CLOSED，继续按等待协议处理，也不得补猜测值。基础设施或计费故障保留原始证据并标 `BLOCKED`，不得伪装通过；忽略无关的 `chatgpt-codex-connector` usage-limit 噪音。

`CLOSED` 定义为：PR 已创建、远端 SHA 对拍、最终 HEAD validator PASS、必需 review 无 blocker/major、e2e 与相关 checks 全绿、PR body 模型字段完整。除非用户另行授权，不自动 merge。

## CLOSED 清理、claim 生命周期与返工

### 本地闭环清理

PR 开出且 e2e/review 全绿后，主干按固定顺序执行：

1. 记录 PR URL、最终 SHA、结论、测试、validator 和 gate 状态，关闭实施 subagent。
2. 在 worktree 仍存在时执行 `git status --porcelain=v1 --untracked-files=all`，确认没有源码、用户 WIP 或未提交改动，也没有本流程产生的孤儿 stash；不干净则停止清理并派恢复。
3. 识别并删除**明确属于该 worktree、独占、ignored、可再生**的生成目录，再次确认没有源码/WIP。显式排除共享 `CARGO_TARGET_DIR`，绝不任务级清理它。
4. 在控制面执行 `git worktree unlock <path>`，再 `git worktree remove <path>`。
5. 删除对应本地 branch；不得先删仍被 worktree 检出的 branch。
6. 执行 `git worktree prune`，释放槽位并补下一个 skeleton。远端 claim 分支保留给 review 返工/merge。

### 远端 claim 释放与孤儿巡检

- PR merge：核验远端 claim 是否已删；未删时由主干删除并再次核验。
- PR close 且确认放弃：先确认无开放 PR、无存活 subagent、远端提交无需保留，再删除 claim ref，让 skeleton 重新开放。
- claim 成功但 PR 未创建便异常退出/失联：主干确认无开放 PR、无存活 subagent、远端无须保留提交后删除孤儿 claim。每轮补位都巡检一次。
- 其它失败：保留有恢复价值的 worktree/ref，派恢复 subagent；不要盲删 BLOCKED 任务。

### review/e2e 返工责任链

review 或 e2e 出现本分支问题时，主干派**新的返工 subagent**，从同一远端 PR branch 重建并锁定 worktree；不要让主干修，也不要假设旧 worktree 仍在。

返工建树不调用 create-ref。在控制面执行以下幂等链：

1. 确认 PR 仍 open，读取 `pr_head_sha` 与远端 branch 名。
2. `git fetch origin refs/heads/<remote-branch>:refs/remotes/origin/<remote-branch>`，对拍远端跟踪 ref SHA 等于 `pr_head_sha`。
3. 确认目标 worktree 路径和专用本地返工 branch 未被其它任务使用；执行 `git branch --track <专用本地返工branch> origin/<remote-branch>` 创建唯一专用跟踪分支。若同名本地分支已存在，只能在确认未被 worktree 使用且没有需保留的本地提交后删除并重建；不得盲目 reset。
4. `git worktree add --lock <绝对路径> <专用本地返工branch>`，再对拍 worktree HEAD、upstream SHA、远端跟踪 ref、PR head 四者都等于 `pr_head_sha`。
5. 任一步失败时只 unlock/remove 半成品 worktree、删除本轮专用本地 branch并 prune；**开放 PR 的远端 claim ref 不得删除**。

完成四方 SHA 对拍后才进入任务面。

返工必须幂等：不重复 claim、promotion、Finish Evidence 章节或归档移动。按“修复并提交 → 新 HEAD validator → 完整门禁 → fetch 后按 step 6 分类同步最新主线 → 条件复验与新 SHA validator → 原地更新 Finish Evidence（若证据变化）→ 最终 HEAD validator → push 同一分支 → 等新 HEAD e2e → 独立评论 `/review`”完整闭环。返工产生的每个 commit 和最终 PR body 仍必须使用精确模型字段。

## 状态汇报

持续运行时只汇报当前 N 路任务、phase、最终/validator SHA、PR/gate、BLOCKED 原因与空槽，不把大段 diff/日志灌回主干上下文。用户说“停止”后不再领取新任务；让正在进行的破坏性操作安全落点，再报告所有 worktree、branch、claim 和 PR 状态。
