# Bong · AGENTS.md

> 这份文件被 **oh-my-opencode 的 `directory-agents-injector` hook** 自动注入到任何读取本仓库文件的 opencode session 中。内容是给 agent 看的硬约束，不是给人看的使用文档（`CLAUDE.md` 才是）。
>
> 简短原则：**CLAUDE.md 描述项目**，**AGENTS.md 约束 agent 行为**。两者互补，不复制。

---

## 1. plan 消费流水线（`docs/plan-*.md` → Atlas → 归档）

适用于 `/consume-plan`、`scripts/bong-plan-auto.sh`、手动 `@plan` 启动的所有流水线 session。

### 1.1 plan 来源白名单

| 目录 | 状态 | 流水线是否可消费 |
|---|---|---|
| `docs/plan-*.md` | 活跃定稿 plan | ✅ 仅此可消费 |
| `docs/plans-skeleton/*.md` | 仅标题占位（见该目录 README） | ❌ 禁止 |
| `docs/finished_plans/*.md` | 已归档历史 | ❌ 禁止 |

若调用方传入骨架或归档 plan，立即 `<promise>BLOCKED: 不能消费骨架或已归档 plan</promise>` 退出。

### 1.2 运行态 vs 源码态隔离

- `docs/` 是 **source of truth**：流水线**只读取**，**不回写**。
- 运行态全部落在 `.sisyphus/`：
  - `.sisyphus/inputs/<name>.md` —— 从 `docs/` 拷入的 plan 快照
  - `.sisyphus/plans/<name>.md` —— Prometheus 规整输出
  - `.sisyphus/boulder.json` —— Atlas 执行状态（支持中断恢复）
  - `.sisyphus/drafts/` —— Prometheus interview drafts（本场景通常不触发）
- **唯一允许的 docs/ 写入**：所有 TODO 绿 → `bash scripts/plan-finish.sh <name>` → `git mv docs/plan-<name>.md docs/finished_plans/`。

### 1.3 四阶段编排（不可跳过、不可重排）

1. **Prometheus** 把 `.sisyphus/inputs/<name>.md` 视为**已完成的 interview transcript**，规整为 `.sisyphus/plans/<name>.md`。**禁止** interview、**禁止**扩写需求、**禁止**改动 `docs/`（`prometheus-md-only` hook 会强制）。
2. **Metis** 做预分析（hidden intent / AI failure points），结果**回填**到 `.sisyphus/plans/<name>.md` 对应 TODO，不新开文件。
3. **Momus** 以 high-accuracy 模式审核，拒绝 → Prometheus 修正 → 再审。`/ulw-loop` max 100 iter 兜底。
4. **Atlas** 执行 `/start-work <name>`，按 TODO 逐个落地。

### 1.4 失败即标注，不阻断

| 失败粒度 | 行为 |
|---|---|
| 单 TODO 测试红 | `session-recovery` + `ralph-loop` 自动续 |
| 同 TODO 连续 3 轮红 | 在 `.sisyphus/plans/<name>.md` 该 TODO 下标 `[BLOCKED: <原因 + 测试名 + 关键错误>]`，**跳过继续下一个** |
| 全部走完有 BLOCKED | `<promise>BLOCKED: N 项阻塞</promise>`，不归档 |
| 全部绿 | `bash scripts/plan-finish.sh <name>` → `<promise>DONE</promise>` |

---

## 2. 三栈命令矩阵（严格对齐 `CLAUDE.md`）

| 栈 | 目录 | 命令 |
|---|---|---|
| server (Rust) | `server/` | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；运行 `cargo run`（offline mode） |
| client (Java/Fabric) | `client/` | `./gradlew test build`；UI 验证 `./gradlew runClient`（JDK **17**，不是系统默认 21） |
| agent (TypeScript) | `agent/` | `npm run build`；子包 `packages/tiandao` `npm start` / `npm run start:mock` / `npm test`；`packages/schema` `npm test` |
| worldgen (Python) | `worldgen/` | `python -m scripts.terrain_gen`、`bash worldgen/pipeline.sh`；raster 校验走 `worldgen/scripts/terrain_gen/harness/raster_check.py` |
| 联调 | 仓库根 | `bash scripts/dev-reload.sh`、`bash scripts/smoke-test.sh`、`bash scripts/smoke-test-e2e.sh` |

**不要跨栈调命令**（server 里不跑 npm、agent 里不跑 cargo）。

---

## 3. 禁止动作

- `git push --force`、`git reset --hard`、`git commit --amend`（无明确用户授权时）
- `--no-verify`、`--no-gpg-sign`、`-c commit.gpgsign=false`
- 绕过 "Java 17 for Fabric" 约定
- 改 `.gitignore`、`package.json`、`Cargo.toml` 的依赖版本（除非当前 plan 明确要求）
- 向 `docs/worldview.md` 回写（世界观锚点，只在核心 canon 改动时手动修）
- 向 `docs/library/` 主动回写（图书馆域由专门的 `library-curator` agent 负责，plan 流水线不跨界）
- **`git stash push` 无对等 `git stash pop`**：任何在主仓库 auto-stash 的流程，完成时必须把自己产生的 WIP stash pop 回来；不得在主仓库留下 `WIP before inspecting ...` 孤儿 stash（历史教训：曾 stash + `reset --hard` 主仓库但不 pop，用户 worktree 改动凭空"消失"直到从 stash 捞出）

---

## 4. 委派偏好

| 任务 | 目标 |
|---|---|
| 架构 / review / 难 debug | `@oracle`（read-only） |
| 代码库大范围搜索 | `@explore` |
| 多仓 / OSS 实现参考 | `@librarian` |
| UI / 前端视觉 | `delegate_task(category="visual-engineering", ...)` |
| 硬逻辑 / 架构决策 | `delegate_task(category="ultrabrain", ...)`（已配 `openai/gpt-5.4`） |

---

## 5. Commit 约定

- `git-master` skill 负责拆分：3+ 文件 ≥ 2 commits，5+ 文件 ≥ 3 commits
- commit message **中文**，匹配仓库近 30 提交风格（git-master 自动检测）
- 每章节 TODO 绿 → commit；不堆积巨型 commit
- 归档 commit 形如：`归档 plan-<name>：<一句话总结>`
- Bong 关闭 `git_master.commit_footer`（见 `.opencode/oh-my-opencode.json`），保留 `Co-authored-by` 尾签

---

## 6. 零交互

整个流水线不向用户发问、不等确认。歧义点处理顺序：

1. 本文件
2. `CLAUDE.md`
3. `docs/worldview.md`（仅世界观相关歧义）
4. 真正无解 → `[BLOCKED: ...]` 标注，继续其它 TODO，不阻断

---

## 7. 沟通语言

中文。commit、narration、plan 注释、`<promise>` 消息统一中文。
