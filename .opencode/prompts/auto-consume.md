ultrawork high accuracy.

你正在 Bong 仓库的一个 git worktree 里，**全自动** 消费一份 Bong 的定稿开发 plan。**整个流程禁止向我发问，禁止停下来等确认**，所有歧义用 `@CLAUDE.md` 和 `@docs/worldview.md` 的既有约束兜底判断。

---

## 输入

- **plan 输入**：`@.sisyphus/inputs/{{PLAN_NAME}}.md`
  这是从 `docs/plan-{{PLAN_NAME}}.md` 拷贝进来的**定稿 plan**，已含章节、验收标准、交叉引用。
- **项目边界**：`@CLAUDE.md`（项目描述 · 命令矩阵、三栈架构、约定）
- **硬约束**：`@AGENTS.md`（agent 行为规则，`directory-agents-injector` 会自动注入）

---

## 四阶段全自动流水线

### 阶段 1 — Prometheus 规整（**不要 interview**）

把 `.sisyphus/inputs/{{PLAN_NAME}}.md` 视为**已完成的 interview transcript**。Prometheus 的唯一职责是**结构化重写**为 `.sisyphus/plans/{{PLAN_NAME}}.md`：

- 切出原子 TODO（对应 plan 原文的章节/小节）
- 每个 TODO 标注：三栈定位（server/client/agent/worldgen/docs）、验收命令（从 `@CLAUDE.md` 命令矩阵挑）、依赖关系
- **禁止**向我发问、禁止扩写需求、禁止改动 `docs/` 下任何文件（`prometheus-md-only` hook 会强制）
- 如果原 plan 的某个点真的缺关键信息：在 TODO 里标 `[BLOCKED: <原因>]`，继续处理其它 TODO，不要停

### 阶段 2 — Metis 预分析（mandatory）

Prometheus 写完 plan 后，调 Metis 做隐性需求/AI 失败点扫描。Metis 输出的每一条都要回填到 `.sisyphus/plans/{{PLAN_NAME}}.md` 的对应 TODO，**不新增独立文档**。

### 阶段 3 — Momus 审核（high-accuracy 模式）

调 Momus 以 high-accuracy 模式审核。拒绝 → Prometheus 修正 → 再审。循环由 `/ulw-loop` 负责兜底（max 100 iter）。Momus 批准后才能进入阶段 4。

### 阶段 4 — Atlas 执行（`/start-work {{PLAN_NAME}}`）

**立即执行**：
```
/start-work {{PLAN_NAME}}
```

（`auto-slash-command` hook 会自动触发，无需我确认。）

Atlas 按 TODO 逐个实施：

- **三栈命令矩阵**（严格遵循 `@CLAUDE.md`）：
  - server：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - client：`cd client && ./gradlew test build`
  - agent：`cd agent && npm run build`，子包 `npm test`
  - schema：`cd agent/packages/schema && npm test`
  - worldgen：`python -m scripts.terrain_gen` / `bash worldgen/pipeline.sh`
  - 一键联调：`bash scripts/dev-reload.sh` / `bash scripts/smoke-test.sh`
- **委派优先**：
  - 架构/review → `@oracle`
  - 代码库探查 → `@explore`
  - 多仓/OSS 参考 → `@librarian`
- **commit 策略**：`git-master` skill 负责 atomic commit；3+ 文件拆 2+ commits（omo 默认规则）；每章节验证通过才 commit
- **失败处理**：
  - 测试失败 → `session-recovery` + `ralph-loop` 自动续
  - 持续失败（3 轮仍红）→ 该 TODO 标 `[BLOCKED: <测试名> <关键错误>]`，跳过该 TODO 继续下一个，不停
  - 所有 TODO 走完但有 BLOCKED → 发 `<promise>BLOCKED: N 项阻塞，详见 .sisyphus/plans/{{PLAN_NAME}}.md</promise>`
- **完工收尾**：
  - 所有 TODO 绿 → 执行 `bash scripts/plan-finish.sh {{PLAN_NAME}}`（把 `docs/plan-{{PLAN_NAME}}.md` 移到 `docs/finished_plans/`，自动 commit）
  - 发 `<promise>DONE</promise>`，`/ulw-loop` 检测到退出

---

## 硬约束

1. **零交互**：整个流水线不问我任何问题，不等确认。所有决策点按本 prompt + `@CLAUDE.md` + `@AGENTS.md` 判断。
2. **不污染 docs/**：`docs/` 下只能**读取**，以及**最后归档**（由 `scripts/plan-finish.sh` 做 `git mv`）。运行态全部写到 `.sisyphus/`。
3. **尊重三栈边界**：Rust/Java/TS/Python 各走各的命令，不要在 server 里跑 npm、不要在 agent 里跑 cargo。
4. **中文输出**：所有 commit message、narration、plan 注释统一中文，匹配 Bong 约定。
5. **不改 git 配置、不 `--no-verify`、不 force push**（`@CLAUDE.md` 和仓库总约定）。
6. **worktree 隔离已由宿主脚本处理**：你就在一个一次性 worktree 里，可以放开改代码；主工作区不受影响。

---

## 启动

现在：

1. 读 `@.sisyphus/inputs/{{PLAN_NAME}}.md`
2. 进入阶段 1

不要输出确认、不要问我、不要列计划给我看 —— 直接进 `@plan` 流程。
