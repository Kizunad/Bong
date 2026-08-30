---
name: bugfix
description: 持续调度 Bong BugFix 闭环：按用户启动参数并行实施 subagent，在常驻 slot/branch 中完成原子 claim、进驻、promotion、第一性原理证真或证伪、最小修复、完整门禁、合并主线复验、归档、PR、review 与 e2e。用户未指定时默认 2 路实施、gpt-5.6-sol-xhigh 实施。主 agent 只调度、等待、锁运维和清理，不直接修代码。用于 /bugfix、$bugfix、“跑 BugFix”“持续修 bug”“把 bug skeleton 闭环”等请求。
---

# Bong BugFix 主干调度

## 身份与终止条件

作为主干调度 agent 持续 loop，直到用户明确说停止。不要把“暂时没有 skeleton”“等待 CI”或“agent 失败”当成整个 loop 完成。

只执行以下主干职责：

1. 维护任务表与用户指定的实施槽位。
2. 启动、等待、唤醒和关闭实施或返工 subagent。
3. 盯 PR review/e2e，维护 claim ref 生命周期，巡检孤儿锁。
4. 收口已闭环任务：释放常驻 slot（detach + 删本地分支 + 清任务私有生成物，**保留 slot 保温缓存**），巡检遗留一次性 worktree，再立即补位。

禁止主干直接修改业务代码、plan、测试或 PR diff，禁止在主 checkout 提交，禁止 stash/reset 用户工作，禁止替 subagent 创建 claim、push 提交、开 PR 或 merge。删除远端 claim ref 是锁运维，是“不 push”禁令的唯一例外。

## 启动参数、并发与资源

- 启动时读取并保持两个独立参数：实施数量 `N`、实施模型。用户未指定时才默认 `N=2`、实施模型 `gpt-5.6-sol-xhigh`；不得用默认值覆盖用户输入。
- 每个实施 subagent 只负责 1 个 skeleton。实施总槽位按 `N`，但**同时执行编译的 slot/task 始终不得超过 2**；非编译调查槽位不因这个资源上限被错误固定为两路。
- 平台总 agent 槽位纳入启动准入：主干占 1 槽。容量可查时，每次补位按实时快照计算 `min(max(0, N - 当前实施 agent 数), max(0, platform_total - live_agents - 未计入 live 的主干占位))`；容量不可查时只按 `max(0, N - 当前实施 agent 数)` 补位，编译并发仍由独立 token 限制为 2。平台满载、已有无关 agent 或 outstanding reservation 都会减少可启动数；超额任务进 FIFO，不 spawn。
- 用户指定模型不可用时，先按用户明确允许的候选路由；没有允许的替代模型时请求用户决策。只有默认模型不可用且用户未指定时，才说明可用模型并请求选择；不得静默降级，也不得直接阻断整个 loop。
- Claude 在补位前执行 `bash ~/.claude/quota.sh`；GPT/Codex 跳过该脚本，不读取、不申请权限、不因其失败阻塞。
- 所有运行者执行 `df -h /`。磁盘超过 90% 时，主干运行 `bash scripts/wt-janitor.sh`（report-only）并按报告回收遗留：仅 PR **MERGED** 且干净、无非缓存 ignored（`.env` 等）、无未合入 patch 的树用 `--apply` 自动收（squash 后用 `git cherry` 判等价）；CLOSED/UNKNOWN/无 PR/脏树一律交人工。常驻 slot 的 `server/target`/`client/build` 保温缓存**不属于**「私有可再生生成物」，不参与任何任务级清理。
- **严禁任务级删除或 `cargo clean` 共享 `CARGO_TARGET_DIR` 与 slot 保温缓存**。只有主干确认所有编译均已停止后，才可统一处理共享缓存；`cargo clean -p valence_generated` 只用于该前提下的故障恢复（slot 路径恒定后，删 worktree 烙坏 valence_generated 绝对路径的故障不应再出现）。

### harness 能力探测与适配

启动时先枚举当前宿主真实可用工具，只选择一个适配分支；禁止把任一命名空间描述成通用 Codex API，也禁止调用当前分支未暴露的接口。下列 `harness-contract` JSON 是脚本与文档共享的权威适配表，修改后必须运行 dry-run；`required_tools` 必须全部存在才允许选择该分支，不能用 CLI 子命令、说明性映射或相似名字顶替宿主工具。

```harness-contract
{
  "version": 1,
  "adapters": {
    "collaboration": {
      "required_tools": ["spawn_agent", "send_message", "wait_agent", "followup_task", "list_agents"],
      "spawn": "spawn_agent",
      "message": "send_message",
      "wait": "wait_agent",
      "resume": "followup_task",
      "status": "list_agents"
    },
    "multi_agent_v1": {
      "required_tools": ["spawn_agent", "send_input", "wait_agent", "resume_agent", "close_agent"],
      "spawn": "spawn_agent",
      "message": "send_input",
      "wait": "wait_agent",
      "resume": "resume_agent",
      "status": "runtime_status_or_list"
    },
    "claude_code_cli": {
      "required_tools": ["Bash", "Monitor"],
      "adapter": "skills/bugfix/scripts/claude_code_adapter.py",
      "spawn": "Bash adapter spawn",
      "message": "phase-yield result or adapter resume prompt",
      "wait": "Monitor adapter wait",
      "resume": "Bash adapter resume by canonical sessionId",
      "status": "Bash adapter status via claude agents --json --all"
    },
    "claude_tools": {
      "required_tools": ["Agent", "SendMessage", "ScheduleWakeup"],
      "spawn": "Agent",
      "message": "SendMessage",
      "wait": "ScheduleWakeup",
      "resume": "Agent continuation",
      "status": "Agent result and completion notification"
    }
  }
}
```

- `claude_code_cli` 是当前 Claude Code 2.1.207 的生产 adapter：宿主必须暴露 `Bash` 与 `Monitor`，并通过 `python3 skills/bugfix/scripts/claude_code_adapter.py` 调用真实 CLI。`spawn` 使用 `claude --background`；`status` 使用 `claude agents --json --all` 并以完整 `sessionId` 作为 canonical ID；`resume` 使用 `claude --resume <sessionId> --background`；`wait` 必须把 adapter 的有限 `wait` 子命令交给宿主 `Monitor`，由退出/超时事件唤醒主干。该 CLI 没有父子 agent 双向 message API，因此 token 协议强制 phase-yield：子任务在结果中返回结构化请求后结束，主干再把 grant/checkpoint 放入 resume prompt。
- `claude_tools` 只适用于宿主在**当前会话工具清单**中真实暴露全部三个工具的旧 Claude harness；不得把 `claude_code_cli` 的 CLI 命令冒充这些工具。Claude Agent SDK 若由产品接入 Managed Agents，可用 session/thread event stream 实现 spawn/message/status/wait/resume，但必须另写可执行 adapter 并验证事件链，不能只凭 SDK 文档选中分支。
- 当前宿主缺任一 `required_tools` 时必须 fail closed：记录缺失能力并停止启动 BugFix loop，不得执行部分状态机。尤其等待能力缺失时不能用 shell sleep/busy-poll 冒充。
- 修改 `claude_code_cli` adapter 或契约时，除纯内存 dry-run 外必须运行 `python3 skills/bugfix/scripts/claude_code_adapter_test.py`；该测试驱动真实 `claude --background` → `claude agents --json` → 有限 wait → `claude --resume`，并清理探针 session。provider 429 时至少退避 120 秒再重跑。
- 支持子 agent 主动发消息的 harness：用本分支真实 message API 发送结构化 token 消息；主干用本分支真实 wait/status API 接收和对拍。
- 不支持子 agent 在运行中等待父级输入，或能力探测不确定时：强制使用 **phase-yield/checkpoint**。实施 agent 结构化返回 `TOKEN_REQUEST + checkpoint` 后结束当前 turn，主干入 FIFO；获准后通过该 harness 的 `resume_agent` / `followup_task` / `send_input` / Agent continuation 恢复同一任务。不得假设子 agent 能主动 `wait_agent` 等父级。
- 能力不足以恢复同一任务时，持久化绝对 worktree、phase、HEAD、request/token、测试证据，启动同任务 continuation；不得传实施结论或丢失 checkpoint。

### 主干唯一 token 准入协议

主干是资源状态表、等待队列和授权消息的**唯一持有者**，串行处理申请，禁止 subagent 自行计数或先做后报：

| token | 容量 | 持有阶段 | 状态表字段 |
|---|---:|---|---|
| `compile_token` | 2 | 进入任何编译/完整门禁前至该轮命令结束 | token id、task、agent、grant time、phase |

按上方适配器选择真实接口完成可恢复握手：

1. 实施 agent 发送或 phase-yield 返回：`TOKEN_REQUEST{"request_id":"<uuid>","type":"compile","task":"<id>","agent":"<canonical agent id>","phase":"<phase>","head":"<sha>","generation":N,"checkpoint":"<resume data>"}`；**未收到有效 grant 禁止编译**。
2. 主干按 `request_id` 幂等去重，落唯一状态表、排入 FIFO。grant 前重新读取权威 task 的 task/phase/head/generation；逻辑容量与实时平台槽位都满足时，先原子登记 holder，再返回 `TOKEN_GRANTED{"request_id":"...","token_id":"...","type":"...","task":"...","phase":"...","head":"...","generation":N,"expires_at":"..."}`。
3. 实施 agent 核对 request/task/phase/head/generation/expiry 后发送或返回 `TOKEN_ACK{...}`，ACK 后 token 才可使用。主干对**每次** ACK（包括重复 ACK）重新读取权威状态；任何漂移都原子 stale/cancel。重复 request 返回同一队列状态或同一 grant；状态未漂移时重复 ACK 幂等。
4. 已结束/idle agent 通过当前 harness 的 resume/followup 入口恢复，payload 必须包含 `checkpoint + request_id + agent + phase + head + generation`；进入 `RECOVERING` 前、生成每一份 followup payload 前、处理恢复结果时都重新读取权威 task，任一 HEAD/generation/来源 phase 漂移立即 stale/cancel 且不再占 token。任务权威态允许且只允许 `task.phase=RECOVERING && recovery_from=grant.phase` 与 broker RECOVERING 临时对应；恢复结果逐字段回传并与原 grant 对拍。禁止另开无状态 agent 猜测续点。
5. 完成、FAIL、超时、异常时，实施 agent 发送或返回绑定 `request_id + agent + phase + head + generation + reason` 的 `TOKEN_RELEASED{...}`；排队取消或 grant 失效返回绑定原 request 身份与 reason 的 `TOKEN_CANCELLED{...}`。主干确认后释放并回 `TOKEN_RELEASE_ACK{...}`；同 request 的重复终态消息只有完整 payload 相同才幂等。
6. grant 绑定 request_id、task、agent、phase、head、generation。任一变化、token 已回收、非 FIFO 当前授权或 ACK 前过期都原子 stale；旧 token 禁止 ACK/recovery/release。
7. 重复 RELEASE/CANCEL 必须幂等 no-op；同 request_id 但 payload 不同视为协议错误。每次唤醒用当前 harness 实际状态查询对拍 holder；失联先进入 `RECOVERING`，每轮 recovery sweep 都先对拍权威 task，漂移立即 stale/cancel；状态未漂移时只有 resume/send-input/followup 明确失败或恢复 TTL 到期才回收。回收后的迟到消息一律拒绝。
8. `compile_token` 在该轮门禁任一出口释放。任务 BLOCKED/CLOSED、用户停止或 worktree 清理前，主干取消排队项并核验无悬挂 token。

从仓库根运行 `python3 skills/bugfix/scripts/state_machine_dry_run.py` 验证这套状态机。该 dry-run 不调用 GitHub 写 API，覆盖容量/FIFO、消息握手、异常回收、claim/main-sync 与持续等待契约；修改资源协议时必须同步更新并运行。

## 启动上下文与任务表

在任何 skeleton 四查、claim 或 promotion 之前，主干与实施/返工 subagent 都必须无条件完整读取 `docs/CLAUDE.md`；根 `AGENTS.md`、`CLAUDE.md` 与 `docs/plans-skeleton/README.md` 同时读取，但不能替代 `docs/CLAUDE.md`。worktree 建立后在任务面再次读取这些文件；涉及 gameplay/真元时额外读取 `docs/worldview.md`。

维护唯一任务/资源状态表；以下字段不得省略：

| skeleton | agent | worktree | branch | phase | generation | token_type | request_id | token_id | queue_pos | holder | request_phase | requested_at | granted_at | expires_at | ack | recovery_deadline | release_reason | checkpoint | head | gate evidence | PR | review/e2e |
|---|---|---|---|---|---:|---|---|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

phase 只使用：`DISPATCHED → CLAIMED → PROMOTED → VERIFYING → FIXING/NOT_BUG → GATING → REBASING → ARCHIVING → PR_OPEN → GATES → CLOSED`，失联暂态 `RECOVERING`，或终态 `BLOCKED`；合法边、必经里程碑、互斥分支、终态和返工回流以 dry-run 的 `TaskPhase/ALLOWED_EDGES` 为可执行契约。

状态迁移不能靠“走过 phase”解锁：每个阶段都必须保存与当前 `head`、`generation` 对应的 gate evidence；`GATING → REBASING` 必须有当前 HEAD/generation 的成功 gate evidence，主线同步也必须记录当前 HEAD/generation 的成功 sync evidence。主线同步、HEAD 变化或任何返工回流会递增 generation，并清除旧 SHA/generation 的 gate/sync 证据；进入 `PR_OPEN/GATES`（包括从其进入 RECOVERING）后禁止原地改 HEAD，必须先走 `GATES → FIXING/NOT_BUG` 返工边。FAIL、缺证据、错误 SHA 或旧 generation 均不得推进；`GATES → CLOSED` 必须核验当前非空 HEAD/generation 的 PR number、与权威 repository/number 精确拼出的 PR URL、远端 SHA、无重复名称的 review/e2e/相关 checks 均为 SUCCESS，以及与启动时实施模型一致的规范精确 ID，禁止用聚合布尔值或自报模型代替证据。

## 选择与派发 skeleton

1. `git fetch origin` 后只从 `origin/main:docs/plans-skeleton/plan-bughunt-*.md` 选择疑似 bug；优先用户指定项，否则按可达性、影响和局部可修性排序。
2. 派发前四查仅作辅助诊断：skeleton 仍存在、无同名 active plan、目标未被已合并修复覆盖、无同名远端分支/开放 PR。查询存在 TOCTOU，**不得把四查当互斥锁**。
3. 一个 skeleton、subagent、常驻 slot 进驻、branch、PR 必须一一对应（slot 是复用工作目录，不是每任务新建 worktree）。主干只传 skill 路径、skeleton 路径、任务 ID、绝对 slot 目标路径和模型要求；不要传真伪判断或预设修法。
4. 没有可领取项时进入等待；不要制造 skeleton 充数。

## 等待协议

- 等待新 skeleton、资源槽、review 或 e2e 时采用约 1200 秒节奏：`claude_tools` 使用 `ScheduleWakeup`；`claude_code_cli` 使用宿主 `Monitor` 执行 adapter 的有限 `wait` 子命令；Codex 使用产品 wait/monitor 或 goal continuation；其它 harness 使用其真实等待/续跑入口。不要声称未暴露的 harness 具有 `ScheduleWakeup`。
- 同一等待对象连续 3 轮无进展时升级告警、记录证据并继续对应 harness 的等待机制；不得因此结束 loop。只有用户明确停止，或出现确定性且必须由用户选择的决策点，才暂停等待输入。
- 禁止 shell `sleep` loop、短周期 GitHub 查询和持续占用 agent 的 busy-poll。

## 实施 subagent 强制状态机

### 1. 原子 claim 与进驻常驻 slot worktree

subagent 是 claim ref 的**唯一创建主体**。分支固定为 `bugfix/<plan-basename>`。worktree 不再按任务新建，改为进驻主干分派的**常驻编译 slot**：`.agent-worktrees/slot-<k>`（k=1..N，N 由 `scripts/slot_registry.sh init --max N` 固定，默认 2；主干按需一次性创建、永久 locked）。占用必须先过 `slot_registry.sh acquire` 原子 reservation，detached HEAD 不能单独充当空闲信号。slot 跨任务保温 `server/target`/`client/build` 增量缓存——这是磁盘占用上限固定（不随任务数增长）与热编译的核心，**任何阶段都严禁 remove slot 或删除其构建缓存**。

严格区分作用域：

- **控制面**：以下 claim、fetch、slot 创建、失败回滚命令必须在已经存在的主仓库绝对路径或专用调度目录执行。
- **任务面**：只有 slot 进驻完成、locked、upstream 与三方 SHA 对拍完成后，才允许在 slot 绝对路径中读取任务代码、编辑、测试、commit、push 和操作 PR。

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
5. 进驻主干分派的 slot（**占用权威 = `scripts/slot_registry.sh` 原子 reservation**，不是 detached HEAD。detached 只作辅助诊断：占用中的 slot 通常检出任务分支，空闲 slot 通常 detach；但无 registry 持有时禁止 checkout。容量由 `slot_registry.sh init --max N` 固定，默认 N=编译并发上限 2）：
   - slot 尚不存在时，由主干在控制面用单条 `git worktree add --lock --detach .agent-worktrees/slot-<k> origin/main` 创建（常驻、永久 locked，之后不再重建）。
   - **先 acquire**：`out=$(bash scripts/slot_registry.sh acquire --slot slot-<k> --task <plan-basename> --branch bugfix/plan-X --claim-sha "$claim_sha" --agent <canonical-id>)`，从这次 stdout 提取 opaque `OWNER_TOKEN` 到仅 holder 可见的 `owner_token`（默认 status 不暴露）。失败（busy / capacity full / 非法参数）→ 换 slot 或排队，**禁止**对未持有 reservation 的 slot 做任何写/checkout。
   - **双门核验**：① `git -C <slot绝对路径> symbolic-ref -q HEAD` 无输出（detached；检出着任何分支 = 物理上已被占用，即使 registry 异常也 fail-closed）② `git -C <slot绝对路径> status --porcelain=v1 --untracked-files=all` 输出为空。任一不满足即 `rollback` 释放 reservation、回报主干处置，**禁止自行 clean/reset/切分支动他人数据**。
   - **ignored 安全门**：进驻前枚举 ignored（`git status --porcelain=v1 --untracked-files=all --ignored=matching`）。仅缓存白名单 `server/target`、`client/build`、`client/.gradle` 可接受；出现 `.env`、私有日志等非白名单 ignored → `rollback`、转人工，不得覆盖或删除。
   - **本地分支进驻（避免 `checkout -B` 覆盖残留提交）**：
     - 本地 `refs/heads/bugfix/plan-X` **不存在**：才允许 `git -C <slot绝对路径> checkout -B bugfix/plan-X origin/bugfix/plan-X`，并显式设置 upstream 为 `origin/bugfix/plan-X`，随后 `bash scripts/slot_registry.sh mark-created-local --slot slot-<k> --task <plan> --agent <canonical-id> --owner-token "$owner_token" --value true`。
     - 本地分支**已存在**：禁止 `checkout -B`（会 reset 到远端、覆盖/丢弃残留本地提交）。改为 `git -C <slot绝对路径> checkout bugfix/plan-X`，核验 `rev-parse HEAD` 与 `claim_sha` 一致；不一致 → 转人工，不得强行对齐；`created_local_branch` 保持默认 `false`。
     - BLOCKED 干净释放后的恢复进驻走普通「acquire + 既有本地分支 + SHA 对拍」；脏现场仍占 frozen reservation 时，必须先按本节 audited handoff 取得该 reservation 的新 token，再走「既有本地分支 + SHA 对拍」，禁止 `-B`。
6. 对拍 `git -C <slot绝对路径> rev-parse HEAD`、本地 upstream SHA、远端 claim SHA 三者都等于 `claim_sha`，检查 slot 确实处于 locked 状态，然后只运行 `bash scripts/slot_registry.sh occupy --slot slot-<k> --task <plan> --agent <canonical-id> --owner-token "$owner_token"`；该 executable gate 会重验 canonical slot、registered+locked、branch/HEAD/upstream/claim、dirty/untracked/ignored，不能用前面的手工检查替代。
7. create-ref 成功后的失败回滚分两种（**均不得无条件 `branch -D`**）：
   - **双门/ignored/acquire 后核验失败**（checkout 尚未执行）：不得在 slot 内执行任何写操作；`bash scripts/slot_registry.sh rollback --slot slot-<k> --task <plan> --agent <canonical-id> --owner-token "$owner_token"`（此时 `DELETE_LOCAL_BRANCH` 必为 false），回报主干处置/换 slot。远端 claim ref 仅在本 subagent 本轮刚创建、PR 尚未创建、且删除前重新查询确认其 SHA 仍严格等于本轮 `claim_sha` 时，才由本 subagent 删除并核验不存在；任一条件不满足即保留 ref 交主干。
   - **checkout 之后的步骤失败**（跟踪、SHA 对拍等）：在 slot 内 `git checkout --detach origin/main` 脱离；执行带同一 `--slot/--task/--agent/--owner-token` 的 `slot_registry.sh rollback` 并**仅当 stdout 含 `DELETE_LOCAL_BRANCH=true`（本轮 `mark-created-local true`）才删除本地分支**；既有分支（含 SHA 冲突/BLOCKED 残留）一律保留并交人工。远端 claim ref 仍只适用上一项完全相同的三条件失败回滚；其它释放、巡检与删除统一由主干负责。
   两种路径都不得留下孤儿锁，**不得 remove slot**。正常闭环释放：`detach` →（CLOSED）`branch -D` → `bash scripts/slot_registry.sh release --slot slot-<k> --task <plan> --agent <canonical-id> --owner-token "$owner_token"`。BLOCKED 干净释放：`detach` + **保留本地分支** + 带同一完整 owner identity 的 `slot_registry.sh release`；脏现场：`bash scripts/slot_registry.sh freeze-blocked --slot slot-<k> --task <plan> --agent <canonical-id> --owner-token "$owner_token"` 并交人工。恢复前先运行 `manual-report`；人工随后以完整旧 reservation identity、`--recovery-agent <new-canonical-id>`、operator、reason 执行 audited `force-unfreeze-blocked`。该命令只准备 durable private handoff + public intent，不修改 reservation；从 stdout 提取并安全保存一次性 `OPERATION_ID` 与新 `OWNER_TOKEN`（不得写日志/audit），然后由恢复者携同一 operation/token/operator/reason 调用 `resume-unfreeze-blocked`。resume 按 agent→token→state→completion audit→private cleanup 续跑，任一中断点都可用同一凭据恢复；private handoff 已落盘而 public intent 缺失时普通命令 fail-closed，合法 resume 会先补写并 fsync intent 后才 mutation。status/report/audit 不泄漏 raw token（public audit 只存新 token SHA-256）。reserved 来源最终回到 reserved，恢复者持新 token 继续现有 reservation，完成现场核验后执行 `occupy`，**不得再 acquire**；occupied 来源最终回到 occupied，恢复者持新 token 直接接管 authority。全程不宣称 PID/liveness 自动恢复。

进入任务面后，所有任务 read/edit/test/git/gh 命令都显式在 slot 绝对路径内执行；编译必须落在 slot 自身的 in-tree target（若环境设有全局 `CARGO_TARGET_DIR`，门禁命令前显式 `unset CARGO_TARGET_DIR`），保证保温缓存留在 slot 内。禁止进驻他人正占用的 slot，禁止修改主 checkout。

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

### 4. 完整本地门禁

实施 subagent 必须先申请并收到主干的 `compile_token` 明确授权，才能按所有受影响栈在正确目录运行完整门禁：

- server：`scripts/build-token.sh cargo fmt --check && scripts/build-token.sh cargo clippy --all-targets -- -D warnings && scripts/build-token.sh cargo test`
- client：`scripts/build-token.sh gradle test build`，严格使用 JDK 17
- agent/schema：运行对应包 `npm test`；schema src 改动先 `cd agent && npm run build -w @bong/schema`
- worldgen：从仓库根运行 `bash scripts/dev-reload.sh`
- 跨栈修复：运行所有受影响栈门禁；需要完整联调时运行 e2e，不用 snapshot/smoke 冒充

管道命令必须保留真实退出码（例如检查 `${PIPESTATUS[0]}`），不得把本分支引入的失败称为 pre-existing。

门禁若产生 tracked/untracked 文件，先分类：PR 所需产物必须单独提交并重新跑受影响门禁；纯私有且可再生的 ignored 产物可保留到闭环清理。任何未分类或未提交改动都阻止继续推进。

### 5. 同步最新主线并复验

门禁全绿后执行 `git fetch origin`，立即用 merge-base 分类，不允许直接运行会默认生成提交的 `git merge origin/main`：

- **already-up-to-date**：`origin/main` 已是 HEAD 祖先，不改 HEAD，保留当前门禁证据。
- **fast-forward**：HEAD 是 `origin/main` 祖先，执行 `git merge --ff-only origin/main`。fast-forward 没有 agent 新建 commit，因此无需新增 trailer；HEAD 变化后重跑受影响栈完整门禁。
- **diverged**：执行 `git merge --no-commit --no-ff origin/main`，禁止自动 commit。解决冲突后，先持有 `compile_token` 对未提交的合并结果运行受影响栈完整门禁；通过后用中文消息和精确 trailer 显式提交，例如 `git commit -m "合并主线：复验 plan-X 修复" -m "Model: <实际实施模型精确 id>"`。

任何同步带入变化都必须重跑受影响栈完整门禁；冲突或触及修复相关文件时扩大针对性复验。

三种同步分支收口后都进入 `ARCHIVING`；不得从未通过本地门禁的状态直接开 PR。

任何返工或新提交都重新跑受影响栈门禁，并按本节同步最新主线。

### 6. Finish Evidence 与归档

只有 step 4–5 全部对当前代码成立且不存在 `[BLOCKED: ...]` 时才归档：

1. 把 plan 所有阶段标成 `✅ YYYY-MM-DD`。
2. 填写严格命名的 `## Finish Evidence`，包含落地清单、关键 commit、完整测试结果、跨栈核验、遗留/后续。
3. 运行 `bash scripts/plan-finish.sh <name>`，确认它把 active plan 移到 `docs/finished_plans/`。
4. 以独立中文归档 commit 提交，并带精确 `Model:` trailer。

归档是开 PR 前最后一次允许的 mutation。归档 commit 后重新跑受影响栈门禁；若 review 返工，只原地更新现有 Finish Evidence/归档文件，不重复 promotion、追加第二份 Finish Evidence 或再次移动文件。

### 7. Push、PR 与 gates

push 前再次要求 `git status --porcelain=v1 --untracked-files=all` 为空，并确认所有预期改动均已提交。用 `git log origin/main..HEAD` 检查本分支新增的**每一个 commit**都存在且仅使用真实精确 id 的 `Model:` trailer；缺失、空值、`AI`、`agent`、`unknown` 等值一律阻止 push。

只 push 该最终 HEAD，并确认远端 SHA 与本地一致。创建中文 PR，标题/body 带完整 plan basename；body 必须附证真/证伪结论和完整测试。创建时写入实施模型字段：

```text
Model: <实际实施模型精确 id>
```

PR 创建和 push 新提交后由 Kody 自动 review，不发送手动 `/review` 或 `/review-next` 评论；发现问题或需要针对最新 HEAD 复审时，才执行 `gh pr comment <PR> --body "@kody review --force"`。等待 Kody、e2e 与相关 checks。

闭环前重新读取 PR body，严格校验 `Model` 字段为实际精确 id；基础设施或计费故障保留原始证据并标 `BLOCKED`，不得伪装通过；忽略无关的 `chatgpt-codex-connector` usage-limit 噪音。

`CLOSED` 定义为：PR 已创建、远端 SHA 对拍、必需 review 无 blocker/major、e2e 与相关 checks 全绿、PR body 模型字段完整。除非用户另行授权，不自动 merge。

## CLOSED 清理、claim 生命周期与返工

### 本地闭环清理

PR 开出且 e2e/review 全绿后，主干按固定顺序执行：

1. 记录 PR URL、最终 SHA、结论、测试和 gate 状态，关闭实施 subagent。
2. 在 slot 内执行 `git status --porcelain=v1 --untracked-files=all`，确认没有源码、用户 WIP 或未提交改动，也没有本流程产生的孤儿 stash；不干净则停止清理并派恢复。
3. 识别并删除**明确属于该任务、独占、ignored、可再生**的非缓存生成物（`.tmp` 日志、临时导出等），再次确认没有源码/WIP。**保留 slot 的 `server/target`/`client/build` 保温缓存**，显式排除共享 `CARGO_TARGET_DIR`，绝不任务级清理它们。
4. 在 slot 内执行 `git checkout --detach origin/main` 脱离任务分支；slot 保持 locked，**不 remove、不 prune**。
5. 删除对应本地 branch；不得先删仍被 slot 检出的 branch（顺序：先 detach 后删）。
6. 标记 slot 空闲并补下一个 skeleton。远端 claim 分支保留给 review 返工/merge。旧流程或异常残留的一次性 worktree 由主干用 `bash scripts/wt-janitor.sh` 巡检回收（MERGED+安全门通过才自动；CLOSED/未知/含非缓存 ignored 交人工），不逐个手工追。

### 远端 claim 释放与孤儿巡检

- PR merge：核验远端 claim 是否已删；未删时由主干删除并再次核验。
- PR close 且确认放弃：先确认无开放 PR、无存活 subagent、远端提交无需保留，再删除 claim ref，让 skeleton 重新开放。
- claim 成功但 PR 未创建便异常退出/失联：主干确认无开放 PR、无存活 subagent、远端无须保留提交后删除孤儿 claim。每轮补位都巡检一次。
- 其它失败：保留有恢复价值的现场/ref，派恢复 subagent；不要盲删 BLOCKED 任务。**BLOCKED 不占死 slot**：现场以 commit 形式留在任务分支上（工作区干净）后，主干在 slot 内 `git checkout --detach origin/main`、**保留本地分支**（未推送的提交仍在分支上）、释放 slot。工作区不干净的 BLOCKED 现场冻结该 slot 交人工——这是唯一允许 slot 被长期占用的情形，主干在状态表持续告警。脏现场恢复者先经 audited `force-unfreeze-blocked --recovery-agent <new-id>` handoff 取得轮换后的 `OWNER_TOKEN`；reserved 来源不再 acquire，使用这份 token 完成 slot/branch/checkpoint 核验并过 `occupy`，occupied 来源直接接管。**BLOCKED 恢复进驻走既有本地分支**：在 slot 内 `git checkout bugfix/plan-X`（直接检出保留的本地分支），检出后与状态表记录的 checkpoint SHA 对拍；**禁止 `checkout -B ... origin/...`**——-B 会把本地分支重置到远端位置，远端不含 BLOCKED 的本地提交时现场即刻丢失。step 5 的 `checkout -B` 仅用于全新 claim 进驻。

### review/e2e 返工责任链

review 或 e2e 出现本分支问题时，主干派**新的返工 subagent**，从同一远端 PR branch 进驻空闲 slot；不要让主干修，也不要假设原任务的进驻状态仍在。返工与新实施**共用同一 slot 池**：无空闲 slot 时返工任务排队并**优先于新 skeleton 派发**获得下一个释放的 slot；不得为返工中断在跑任务或强占其 slot。

返工进驻不调用 create-ref。执行以下幂等链：

1. 确认 PR 仍 open，读取 `pr_head_sha` 与远端 branch 名。
2. `git fetch origin refs/heads/<remote-branch>:refs/remotes/origin/<remote-branch>`，对拍远端跟踪 ref SHA 等于 `pr_head_sha`。
3. 主干分派一个空闲 slot；返工 subagent 必须先执行 `slot_registry.sh acquire --slot <slot> --task <plan-basename> --branch <remote-branch> --claim-sha "$pr_head_sha" --agent <canonical-id>`，从本次 stdout 提取并私有保存 `OWNER_TOKEN`。acquire 失败则换 slot/排队，禁止 checkout；成功后核验 slot detached、`git status --porcelain=v1 --untracked-files=all` 为空且 ignored 仅含缓存白名单，并确认专用本地返工 branch 未被其它任务使用。若同名本地分支已存在，禁止删除、重建或 reset；只能在确认未被任何 slot 检出后，按第 4 步直接 checkout 并核验 SHA。
4. 本地返工分支不存在时才 `checkout -B <专用本地返工branch> origin/<remote-branch>` 并执行 `mark-created-local --value true`；本地已存在则直接 checkout 并核验 SHA 与 `pr_head_sha` 一致（不一致转人工并保留分支）。显式设置 upstream 并完成 slot HEAD、upstream SHA、远端跟踪 ref、PR head 四方对拍后，必须使用同一 `OWNER_TOKEN` 执行 `slot_registry.sh occupy`；只有唯一生产进驻门成功，才能进入任务面。
5. acquire 后任一步失败都先在 slot 内 detach（若已 checkout），再用同一 `--slot/--task/--agent/--owner-token` 执行 `rollback`；仅 stdout `DELETE_LOCAL_BRANCH=true` 时才删除本轮新建的本地 branch，既有 branch 必须保留并交人工。occupy 后的正常/BLOCKED 干净退出使用同一身份执行 `release`；脏现场只能 `freeze-blocked` 交人工。**开放 PR 的远端 claim ref 不得删除，slot 不得 remove**。

完成四方 SHA 对拍后才进入任务面。

返工必须幂等：不重复 claim、promotion、Finish Evidence 章节或归档移动。按“修复并提交 → 完整门禁 → fetch 后同步最新主线 → 条件复验 → 原地更新 Finish Evidence（若证据变化）→ push 同一分支 → 等新 HEAD e2e 与自动 review → 发现问题时发送 `@kody review --force`”完整闭环。返工产生的每个 commit 和最终 PR body 仍必须使用精确模型字段。

## 状态汇报

持续运行时只汇报当前 N 路任务、phase、最终 SHA、PR/gate、BLOCKED 原因与空槽，不把大段 diff/日志灌回主干上下文。用户说“停止”后不再领取新任务；让正在进行的破坏性操作安全落点，再报告所有 worktree、branch、claim 和 PR 状态。
