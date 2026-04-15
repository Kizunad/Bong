# Bong · plan-opencode-workflow-v1

**opencode + oh-my-opencode 全自动 plan 消费流水线**。把 `docs/plan-*.md` 作为单一输入，经 Prometheus → Metis → Momus → Atlas 全自动落地，主模型 `openai/gpt-5.4`，零人工介入。

**落位**：`scripts/bong-plan-auto.sh`（宿主入口）· `.opencode/`（配置与 prompts）· `AGENTS.md`（agent 硬约束，替代 `.claude/rules/`）。

**交叉引用**：
- 项目描述与命令矩阵 → `CLAUDE.md`
- agent 行为硬约束 → `AGENTS.md`（本流水线依赖）
- oh-my-opencode 上游文档 → https://github.com/opensoft/oh-my-opencode
- opencode 上游文档 → https://opencode.ai/docs/

---

## §0 设计轴心

| 原则 | 含义 | 反模式（禁止） |
|---|---|---|
| **借 omo 的现成编排，不自造 agent** | Prometheus/Metis/Momus/Atlas 已经是完备的 plan→execute 分离架构 | 再造 4 个 `plan-reader/architect/executor/auditor` agent，重复上游能力 |
| **docs/ 是 source of truth** | 流水线只**读取** `docs/plan-*.md`，运行态全部落 `.sisyphus/` | Prometheus 回写 `docs/`、Atlas 改 `docs/worldview.md` |
| **worktree 隔离** | 每个 plan 一个 `.worktrees/plan-<name>/` + `auto/plan-<name>` 分支 | 在主工作区直接改，污染正在手写的代码 |
| **零交互** | `bash scripts/bong-plan-auto.sh <name>` 一条命令跑到 `<promise>DONE</promise>` 或 `BLOCKED` | 中途弹 "需要确认吗？" 打断用户 |
| **失败即标注，不阻断** | 单 TODO 连续失败 3 轮 → `[BLOCKED: ...]` 标注 + 跳过 | 第一个失败就整条流水线停 |
| **opencode-native 配置** | 规则在 `AGENTS.md`（吃 `directory-agents-injector`），不用 `.claude/` | 把 opencode 配置塞进 Claude Code 目录 |

---

## §1 架构

```
┌──────────────────────────────────────────────────────────────────┐
│                   scripts/bong-plan-auto.sh                      │
│  (宿主脚本：唯一入口，宿主层全自动)                              │
└─┬────────────────────────────────────────────────────────────────┘
  │
  │ 1. 校验 docs/plan-<name>.md 存在且非骨架/归档
  │ 2. git worktree add .worktrees/plan-<name> -b auto/plan-<name>
  │ 3. cp docs/plan-<name>.md  →  .worktrees/.sisyphus/inputs/<name>.md
  │ 4. 渲染 .opencode/prompts/auto-consume.md（替换 {{PLAN_NAME}}）
  │ 5. cd worktree && opencode run --prompt-stdin < 渲染后的 prompt
  │                                                          │
  ▼                                                          │
  opencode 进程（worktree 内）                               │
  ┌──────────────────────────────────────────────────────────┤
  │ oh-my-opencode 插件启动                                  │
  │ ├─ directory-agents-injector  → 注入 AGENTS.md + CLAUDE.md│
  │ ├─ keyword-detector           → "ulw / high accuracy"    │
  │ │                               激活 ultrawork + Momus   │
  │ ├─ auto-slash-command         → prompt 内 /start-work    │
  │ │                               自动执行                 │
  │ ├─ ralph-loop / session-recovery  → 中断自动续           │
  │ └─ prometheus-md-only         → 禁止 Prometheus 改 docs/ │
  │                                                          │
  │ 四阶段流水线（全部 gpt-5.4）                             │
  │                                                          │
  │   阶段 1：Prometheus (规整，不 interview)                │
  │            .sisyphus/inputs/<name>.md                    │
  │                    ▼                                     │
  │            .sisyphus/plans/<name>.md                     │
  │                                                          │
  │   阶段 2：Metis (mandatory 预分析)                       │
  │            hidden intent / AI failure points             │
  │            → 回填到 .sisyphus/plans/<name>.md            │
  │                                                          │
  │   阶段 3：Momus (high-accuracy 审核)                     │
  │            拒绝 → Prometheus 修正 → 再审                 │
  │            (/ulw-loop max 100 iter)                      │
  │                                                          │
  │   阶段 4：Atlas (/start-work <name>)                     │
  │            ├─ 委派 @oracle / @explore / @librarian       │
  │            ├─ 按 CLAUDE.md 命令矩阵跑测试                │
  │            ├─ git-master atomic commit 每 TODO           │
  │            ├─ 失败 3 轮 → [BLOCKED: ...] 跳过            │
  │            └─ 全绿 → scripts/plan-finish.sh <name>       │
  │                      ├─ git mv docs/plan-<name>.md       │
  │                      │         docs/finished_plans/      │
  │                      └─ commit: "归档 plan-<name>：..."  │
  │                                                          │
  │   <promise>DONE</promise>  或  <promise>BLOCKED</promise>│
  └──────────────────────────────────────────────────────────┘
  │
  │ 6. DONE → git push -u origin auto/plan-<name>
  │           (4 次指数退避重试：2s/4s/8s/16s)
  │        → 打印 "人工 review 后开 PR" 提示
  │
  │ BLOCKED → worktree 保留，boulder.json 记录进度
  │           重跑同条命令 → Atlas 从 checkpoint 续
  │
  ▼
 退出
```

---

## §2 文件清单

```
opencode.json                                 # opencode 根配置：plugin=oh-my-opencode, model=openai/gpt-5.4
.opencode/oh-my-opencode.json                 # omo 配置：agent 模型覆盖、disabled_hooks、categories
.opencode/prompts/auto-consume.md             # 四阶段启动 prompt（{{PLAN_NAME}} 由 bong-plan-auto.sh 渲染）
.opencode/commands/consume-plan.md            # opencode 内快捷入口 /consume-plan <name>
.opencode/commands/plan-list.md               # /plan-list 列活跃 plan
AGENTS.md                                     # agent 行为硬约束（directory-agents-injector 自动注入）
scripts/bong-plan-auto.sh                     # 全自动宿主入口
scripts/plan-finish.sh                        # Atlas 调用归档 docs/plan-*.md → finished_plans/
.gitignore                                    # 追加 .sisyphus/ 和 .worktrees/
docs/plan-opencode-workflow-v1.md             # 本文件
```

**历史决策**：最初版把规则放在 `.claude/rules/bong-plan-consumer.md`，走 omo 的 `rules-injector` hook（Claude Code 兼容层）。后改为 `AGENTS.md`（走 `directory-agents-injector`），理由：(1) opencode-native，不依赖 Claude Code 兼容层；(2) 传播覆盖面更广（Atlas 委派出去的子 agent session 也会拿到）；(3) 和 `CLAUDE.md`（项目描述）职责对仗 —— AGENTS.md 只约束 agent 行为。

---

## §3 模型挂载策略

主模型 `openai/gpt-5.4` 贯穿 **plan 消费链路**，其余 omo 默认分工保留：

| 节点 | 模型 | 位置 |
|---|---|---|
| Sisyphus（主编排） | `openai/gpt-5.4` | `.opencode/oh-my-opencode.json → agents.Sisyphus.model` |
| Atlas（plan 执行器） | `openai/gpt-5.4` | `agents.Atlas.model` |
| Prometheus（规划） | `openai/gpt-5.4` | `agents."Prometheus (Planner)".model` |
| Metis（咨询） | `openai/gpt-5.4` | `agents."Metis (Plan Consultant)".model` |
| Momus（审核） | `openai/gpt-5.4` | `agents.Momus.model` |
| category `ultrabrain` | `openai/gpt-5.4` | `categories.ultrabrain.model`（硬逻辑 delegate） |
| oracle / librarian / explore | omo 默认（按 provider priority 路由） | 分工性价比，无需统一 |

顶层 `opencode.json.model = "openai/gpt-5.4"` 作为兜底默认。

---

## §4 自动化机制一览（全部 omo 原生，不自造）

| 机制 | 作用 |
|---|---|
| `/ulw-loop` 命令 | 自引用循环，max 100 iter，检测 `<promise>DONE</promise>` 退出 |
| `keyword-detector` hook | `ulw` / `high accuracy` 激活 ultrawork + Momus |
| `auto-slash-command` hook | prompt 里 `/start-work X` 自动执行，不等手动输入 |
| `session-recovery` hook | API 错误 / 空消息 / thinking block 错误自动恢复 |
| `ralph-loop` hook | Stop 事件时自动续跑 |
| `boulder.json` | session 中断恢复，worktree 内持久化 |
| `git-master` skill | atomic commit（3+ 文件 ≥ 2 commits），自动匹配仓库 commit 风格 |
| `prometheus-md-only` hook | 强制 Prometheus 只能写 `.sisyphus/` |
| `directory-agents-injector` hook | 向上游走注入所有 `AGENTS.md` |
| `claude-code-hooks` hook | 继续跑 Bong 现有 `.claude/settings.local.json` 的 ruff PostToolUse |

**被关掉的 hook**（云端环境 noisy / CI 稳定性）：`startup-toast`、`auto-update-checker`、`session-notification`、`background-notification`、`agent-usage-reminder`。

---

## §5 使用方式

### 5.1 全自动（推荐）

```bash
# 例：消费 docs/plan-HUD-v1.md
bash scripts/bong-plan-auto.sh HUD-v1
```

退出码：
- `0` = Atlas 完成、plan 已归档、已 push origin/auto/plan-HUD-v1
- `2` = BLOCKED，worktree 保留待人工介入
- 其它 = 基础设施错误（opencode 缺失、push 失败等）

### 5.2 opencode 内手动触发

```
/consume-plan HUD-v1
```

（如果不在 worktree 内，会提示先走 5.1 宿主脚本。）

### 5.3 查询活跃 plan

```
/plan-list
```

### 5.4 中断恢复

直接重跑同条命令：

```bash
bash scripts/bong-plan-auto.sh HUD-v1
```

- worktree 已存在 → 复用
- `.sisyphus/boulder.json` 存在 → Atlas 从 checkpoint 续
- `.sisyphus/plans/<name>.md` 存在 → Prometheus 跳过规整阶段

### 5.5 清理

成功 merge PR 后：

```bash
git worktree remove .worktrees/plan-HUD-v1
git branch -d auto/plan-HUD-v1
```

---

## §6 验收标准

| # | 项 | 验证方式 |
|---|---|---|
| 1 | `opencode` 启动时能识别 `oh-my-opencode` 插件且不报配置错误 | `cd <worktree> && opencode --version` 看插件列表；或 `bunx oh-my-opencode doctor` |
| 2 | 所有 plan 链路节点都解析成 `openai/gpt-5.4` | `bunx oh-my-opencode doctor --verbose` 的 "Model Resolution" 段 |
| 3 | `scripts/bong-plan-auto.sh --help`（误用）打印 usage 并非零退出 | 手动跑 |
| 4 | 传入骨架/归档 plan 立即拒绝 | `bash scripts/bong-plan-auto.sh persistence-v1`（在 `plans-skeleton/` 下）应 exit 66 |
| 5 | 正常消费 + push | 选一个小 plan 全流程跑通，在 GitHub 上能看到 `auto/plan-<name>` 分支 |
| 6 | 中断恢复 | 跑到一半 Ctrl-C，重跑同条命令，`boulder.json` 能续 |
| 7 | BLOCKED 非零退出 | 人为让某个测试持续失败，确认脚本退出码 2、worktree 保留、`.sisyphus/plans/<name>.md` 含 `[BLOCKED:` |
| 8 | Prometheus 不能回写 docs/ | 手动让 Prometheus 尝试 `Edit docs/...`，`prometheus-md-only` hook 应阻止 |

---

## §7 非目标（本版本不做）

- **不自动开 PR**。脚本只 push 分支，PR 由人工 review 后用 `gh pr create` 或 Web UI 开（避免 agent 批量开 PR 骚扰）
- **不自动 merge**。omo 的 `enable_pr_auto_merge` 工具可选，但当前流水线不启用
- **不并发多 plan**（技术上可以 —— 脚本多实例各自一个 worktree —— 但首版不推荐，gpt-5.4 配额消耗大）
- **不自动补 `docs/plans-skeleton/` 骨架**。骨架→活跃 plan 的转化需要人工设计 + `@plan` interview，不走全自动
- **不对接 Bong 现有 `agent/` 的天道 Agent**。本流水线是**开发元层**（agent 写 Bong 的代码），和运行时的 tiandao LLM agent 解耦

---

## §8 已知边界

1. **opencode run CLI flag** (`--prompt-stdin`) 的确切名称依赖 opencode 版本。若启动时报 unknown flag，调整 `scripts/bong-plan-auto.sh` 第 ~120 行（已标注）
2. **gpt-5.4 可用性**：依赖你的 provider 配置（`openai/` 前缀）。若模型不存在，omo 的 provider priority fallback 会自动降级；不希望降级可在 `doctor` 输出里确认并手动锁定
3. **`.sisyphus/` 与 omo 默认路径一致**（其它 omo 项目也用这个目录），不会冲突
4. **AGENTS.md 的规则只在 omo 启用 `directory-agents-injector` 时生效**。若以后手动 disable 此 hook，流水线会失去约束 —— 在 `.opencode/oh-my-opencode.json` 的 `disabled_hooks` 列表里显式排除它
5. **commit footer 已关**（`git_master.commit_footer: false`），但 `Co-authored-by: Sisyphus` 尾签保留 —— 如果这不符合 Bong 提交规范，改 `include_co_authored_by: false`
