# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust 无头 MC 服务器（Valence on Bevy 0.14 ECS，MC 1.20.1 协议 763）
- **client/** — Fabric 1.20.1 微端（Java 17，owo-lib UI）
- **agent/** — LLM "天道" agent 层（TypeScript，三 Agent 并发推演）
- **BongWorldGen** — 独立的 Python/NumPy 地形生成库（<https://github.com/Kizunad/BongWorldGen>）

## Quick commands

```bash
# Server
scripts/build-token.sh cargo fmt --check && scripts/build-token.sh cargo clippy --all-targets -- -D warnings && scripts/build-token.sh cargo test
scripts/build-token.sh cargo run              # 监听 :25565，offline mode

# Client
scripts/build-token.sh gradle test build   # jar 在 build/libs/
scripts/build-token.sh gradle runClient    # 通过 WSLg 启动 MC

# Agent（天道）
cd agent && npm run build                          # 编译 TS
cd agent/packages/tiandao && npm start             # 启动天道 Agent
cd agent/packages/tiandao && npm run start:mock    # mock 模式（无需真实 LLM）
cd agent/packages/tiandao && npm test              # 类型检查 + vitest

# Schema
cd agent/packages/schema && npm test

# Worldgen（独立仓库）
cd ../BongWorldGen
.venv/bin/pip install -e '.[dev]'
.venv/bin/bong-worldgen --width 256 --height 256 --seed 812731 --output generated/demo.npz

# Dev reload (regen + validate + rebuild + restart)
bash scripts/dev-reload.sh
bash scripts/dev-reload.sh --skip-regen            # rebuild only
bash scripts/dev-reload.sh --skip-validate         # 跳过 raster 校验

# Full smoke test
bash scripts/smoke-test.sh
```

## Dev test commands

这些命令只用于本地 / dev 测试场景快速搭建，全部挂在 server brigadier 命令树下；client 通过原版命令树自动获得 Tab 补全，agent 不参与。

> **dev-only**：这些入口会显式绕过 worldview 自然修炼规则和 qi_physics ledger 守恒，不允许复用到生产 gameplay 路径。

| 命令 | 用途 |
|------|------|
| `/meridian open <id>` / `/meridian open_all` / `/meridian list` | 强制打通经脉或查看经脉状态 |
| `/realm set <id>` | 直写玩家境界 |
| `/race set <id>` | 切换玩家种族（走真实 `RaceChange` 两阶段事务：装备门重扫/经脉迁移+休眠登记/qi_max 重算+守恒释放，唯一绕过的是自然修炼流程） |
| `/qi set <value>` / `/qi max <value>` | 直写真元当前值或上限 |
| `/technique list` / `/technique add <id>` / `/technique remove <id>` / `/technique proficiency <id> <value>` / `/technique active <id> <bool>` / `/technique reset_all` | 查看、增删、调熟练度或重置功法 |
| `/give <template_id> [count]` | 给予物品 |
| `/clearinv [pack\|all\|naked]` | 清背包 / hotbar / 装备槽 |
| `/zone_qi set <name> <value>` | 直写区域灵气浓度 |
| `/fog spawn <radius> <density> [duration_ticks]` / `/fog clear <id>` / `/fog clear_all` / `/fog list` | 以自己为中心生成/清除动态雾堤（density ≥ 0.85 触发视距遮蔽） |
| `/kill self` / `/revive self` | 触发玩家死亡 / 复活事件链路 |
| `/time advance <ticks>` | 快进 `CultivationClock` |

## Key dependencies & versions

- Valence: git rev `2b705351`（pinned in Cargo.toml）
- big-brain `0.21`，bevy_transform `0.14.2`，pathfinding `4`
- Fabric: MC 1.20.1，Loader 0.16.10，owo-lib 0.11.2+1.20
- Schema: @sinclair/typebox 0.34
- Agent: openai ^4，ioredis ^5，tsx ^4，vitest ^3

## Architecture notes

- **Server ↔ Agent IPC**：Redis（`bong:world_state` 发布，`bong:agent_cmd` 订阅，`bong:player_chat` 队列）
- **IPC schema**：TypeBox（TS source of truth）→ JSON Schema export → Rust serde structs；共享 `agent/packages/schema/samples/*.json` 双端校验
- **天道 Agent**：三 Agent 并发推演（灾劫/变化/演绎时代），Arbiter 仲裁层负责合并与冲突消解
- **NPC AI**：big-brain Utility AI（Scorer → Action 模式），Position ↔ Transform 同步桥
- **地形生成**：已从本仓库迁出至 [BongWorldGen](https://github.com/Kizunad/BongWorldGen)。Bong 只负责读取生成后的 raster，并在运行时按需生成 chunk；生成器、荒野分类、河流与洞穴数据均由独立库维护。
- **Dev harness**：`scripts/dev-reload.sh` 只负责服务端重建与重启；地形生成和地形数据校验在 BongWorldGen 内执行。
- `#[allow(dead_code)]` on `mod schema` in main.rs — schema 模块用于 IPC 对齐，尚未接入运行时

## Current milestone

**M1 — 天道闭环** ✅（2026-04-13 验收通过：server + agent + client 联跑，聊天栏出现 narration，server 消费 agent_cmd）

| 层 | 状态 |
|----|------|
| Server | MVP 0.1 ✅（草地平台、玩家连接、僵尸 NPC、Redis IPC） |
| Agent | ✅（三 Agent 并发、Context Assembler、Arbiter、WorldModel Redis 持久化、137 单测、端到端联调通过） |
| Client | MVP 0.1 ✅（Fabric 微端、CustomPayload、HUD 渲染） |
| Schema | ✅ 双端对齐 |
| Worldgen | Phase A ✅，LAYER_REGISTRY refactor ✅，Phase B ✅（巨树/洞穴/水体/子表面/平滑/结构物/群系细化） |

## Conventions

- 使用中文沟通
- 云端开发，拉到本地 WSL 测试
- `cargo run` 使用 offline mode（无需 Mojang 认证）
- Client 测试通过 `scripts/build-token.sh gradle runClient`（WSLg，无需单独启动器）
- Java 17 用于 Fabric，系统默认 Java 21（sdkman）
- docs/ 目录存放架构设计文档和路线图，修改前可参考
- Python 文件保存后自动 ruff 格式化（PostToolUse hook，见 `.claude/settings.local.json`）
- 跑会开 worktree 的外部 orchestrator（Codex / Sisyphus 等）之前，先 `git commit -m "WIP"` 把 worktree 改动落盘；跑完 `git stash list` 检查孤儿 `WIP before inspecting ...` / `WIP: stash before inspecting ...`，有就 `git stash pop` 回来（那类 agent 会 auto-stash + `reset --hard` 但不 auto-pop）

## Plan 工作流

> **立新 plan 前先读 `docs/CLAUDE.md`** —— 防孤岛调研流程（必读 finished_plans / active / skeleton + 接入面 checklist + 红旗清单）。本节只讲三态流转和 plan 文件结构。

修仙系统功能落地由 plan 文档驱动。**三态流转**：

- **骨架** `docs/plans-skeleton/plan-<name>-vN.md` — 草案，目标 + P0/P1/... 大致划分
- **Active** `docs/plan-<name>-vN.md` — 实施中，被 `/consume-plan` 消费的对象
- **归档** `docs/finished_plans/plan-<name>-vN.md` — 全部阶段 ✅ 且填好 `## Finish Evidence` 后迁入

### Plan 文件结构（写 plan 时必须遵守）

每份 plan 必须包含：

1. **头部**：一句话主题 + 阶段总览（P0/P1/.../P5 各自 ✅⏳⬜ + 验收日期 `YYYY-MM-DD`）
2. **各阶段块**（P0/P1/...）：每段写出**可核验**的交付物——下游核验工具（`/plans-status` / `/audit-plans-progress` / `/consume-plan`）会按这些抓手 grep 代码
   - 模块名 / 文件路径（如 `server/src/cultivation/`）
   - 类型 / 函数名（如 `struct Tribulation` / `fn breakthrough`）
   - 测试声明（如 "cultivation::* 94 单测"）
   - schema 名 / Redis key / 配置字段（如 `bong:insight_request`）
   - 跨仓库契约 symbol（server↔agent↔client，例 `CultivationDeathTrigger`）
3. **`## Finish Evidence`**（迁入 `finished_plans/` 前必填，章节标题严格如此）：
   - **落地清单**：每阶段对应真实模块/文件路径
   - **关键 commit**：hash + 日期 + 一句话
   - **测试结果**：跑过的命令 + 数量
   - **跨仓库核验**：server / agent / client 各自命中的 symbol
   - **遗留 / 后续**：未在本 plan 范围、依赖其他 plan 的待办

### 状态标记

- `✅ YYYY-MM-DD` — 已完成 + 验收日期
- `⏳` — 进行中
- `⬜` — 未开始
- `🔄` — 代码超前于文档（`/plans-status` 等核验工具标出，提示需补文档）
- `⚠️` — 文档自报已完成但代码未找到（红旗）

### 流转规则

- **骨架 → Active**：人工 `git mv docs/plans-skeleton/plan-x-vN.md docs/plan-x-vN.md`，或基于骨架写新版本号 vN+1。skeleton 不会被 `/consume-plan` 消费
- **Active → Finished**：全部 P ✅ + Finish Evidence 写完后，由 `/consume-plan` 在 PR 末尾 commit 内 `git mv` 入 `finished_plans/`，或人工 mv + commit
- **一个 PR 只动一个 plan**：`/consume-plan` 不允许顺手归档/修改其他 plan

### `/consume-plan` 对 docs/ 的写权限

**仅允许**：在 `docs/plan-$PLAN.md` 末尾追加 `## Finish Evidence`、最终 `git mv` 入 `docs/finished_plans/`。

其他 `docs/` 文件 / `CLAUDE.md` / `worldview.md` 严禁自动改——遇到必须改的情况停下交人工。

## BugFix 工作流（多 agent 并行修复 bughunt skeleton）

bughunt 产出的 `docs/plans-skeleton/plan-bughunt-*.md` 由本工作流消费（feature plan 走 `/consume-plan`，别混）。形态：**1 个主干调度 agent + N 个并行修复 subagent**——N 和 subagent 模型由用户启动时指定（原型默认 2 个，惯例 gpt-5.6 sol xhigh）。整体 loop 直到用户明确叫停：配额接近阈值只暂停（`ScheduleWakeup` 等 reset），**不自行终止**。

### 主干（调度）职责——只调度，不动代码

主干保持上下文干净，只做：分派 skeleton、等待、验收关闭 subagent、清理已闭环 worktree 的生成目录、派发 review 返工、补齐并发。**不 push、不 merge、不开 PR、不直接修代码。**

- 任务清单以 **origin/main** 为准：`git fetch` 后读 skeleton 列表。**防重的权威机制是原子 claim 锁，唯一创建主体是 subagent**（执行命令见 Subagent step 1，主干只派任务、绝不抢先创建 claim ref——两个角色都建 ref 会让正常派发必得 422 死锁）。普通 `git push` 没有 create-only 语义（两会话同 base 时第二个 push 是 no-op 也"成功"），不得用作互斥依据；查询式检查有 TOCTOU 竞态。派发前**四查**只作辅助诊断：① skeleton 在 origin/main 上仍存在 ② 无同名 active plan ③ 目标 symbol 未被已 merge 的修复覆盖 ④ 无同名远端分支 / 开放 PR（`gh pr list --state open --search "plan-X"`）。占用持续到对应 PR merge/close 才解除（promotion 只发生在 subagent 分支上，PR 未合并时 origin/main 依旧满足前三查）
- **一个 skeleton = 一个 subagent = 一个常驻 slot 进驻 = 一个 PR**（对齐「一个 PR 只动一个 plan」；slot 是复用工作目录，不是每任务新建 worktree）
- 编译型任务并发 **≤2**（3 个并行 cargo 编译历史上 OOM + 塞盘）。实施工作区用**常驻 slot 池**（`.agent-worktrees/slot-<k>`，k=1..N，N 默认 = 编译并发上限 2，主干 `bash scripts/slot_registry.sh init --max N` 写入可观察容量；按需 `git worktree add --lock --detach` 创建后永久复用）。**占用权威不是 detached HEAD**，而是 `scripts/slot_registry.sh` 的原子 reservation（只准执行 `scripts/slot_registry.sh acquire`，不得手工 mkdir reservation）：主干/进驻方必须先 `acquire --slot --task --branch --claim-sha --agent` 成功才允许对该 slot 做任何 checkout；detached 只作辅助诊断。进驻契约：① registry 持有 + 双门核验 detached + 工作区干净；② 进驻前枚举 ignored，仅 `server/target`/`client/build`/`client/.gradle` 缓存白名单可接受，其它 ignored（`.env` 等）转人工；③ 本地分支不存在时才 `git checkout -B bugfix/plan-X origin/bugfix/plan-X` 并执行带 `--agent <canonical-id> --owner-token "$owner_token"` 的 `mark-created-local --value true`；本地分支已存在时**禁止 `checkout -B`**，改为直接 `git checkout bugfix/plan-X` 并核验本地 SHA == claim SHA，不一致转人工（`created_local_branch` 保持 false）。所有前置 checkout/跟踪配置完成后，必须把 `acquire` 返回的 opaque `OWNER_TOKEN` 连同 slot+task+agent 传给 `slot_registry.sh occupy`；该命令是唯一生产进驻门，会自己核验 canonical slot 已注册且 locked、branch/HEAD/upstream/claim 对拍、tracked/untracked 干净及 ignored 白名单，成功后才 occupied。释放 = detach 回 origin/main +（CLOSED 路径）删本地分支 + 带 slot+task+agent+owner-token 的 `slot_registry.sh release`。BLOCKED 任务现场以 commit 留在任务分支上后同样 detach + 带完整 owner identity 的 `release`（保留本地分支）；脏现场执行带 slot+task+agent+owner-token 的 `scripts/slot_registry.sh freeze-blocked` 冻结交人工；恢复只允许运行 `manual-report` 后由人工以完整旧 reservation identity + 新 recovery agent + operator + reason 执行 audited `force-unfreeze-blocked`。`force-unfreeze-blocked` 只准备 durable 私有 handoff + 公开 intent，并一次性返回必须安全保存的 `OPERATION_ID` 与新 `OWNER_TOKEN`；它不修改 reservation。恢复者必须携同一 operation/token/operator/reason 调用 `resume-unfreeze-blocked`，事务才按 agent→token→state→completion audit→private cleanup 续跑。任一步中断均保留可续跑状态；private handoff 已落盘但 intent 缺失时，普通命令 fail-closed，合法 resume 必须先补写并 fsync public intent 才能 mutation；status/report/public audit 均不泄漏 raw token，仅 audit 新 token SHA-256。reserved 来源最终回到 reserved 后仍须用新 holder 通过既有 reservation 的 `occupy` 门（**不再调用 acquire**），occupied 来源最终回到 occupied 并由新 holder直接接管 authority。全程不宣称 PID/liveness 自动恢复。容量满（held≥max）拒绝新 acquire。slot 跨任务保温缓存——磁盘上限由 capacity 固定、热编译，**严禁 remove slot 或删其构建缓存**。共享 `CARGO_TARGET_DIR` 时删过 worktree 后若报 `No such file` → `cargo clean -p valence_generated`（故障修复手段，须确认无并行编译在跑；slot 路径恒定后此坑不应再现）
- subagent 回报只带结论（PR 链接 / commit hash / 测试与 gate 证据），不回灌大段 diff/日志进主干上下文
- subagent 闭环（PR 开出 + e2e 绿）后**必须释放 slot 再补位**：关闭 agent → slot 内 `git checkout --detach origin/main` 脱离任务分支 → **`git branch -D bugfix/plan-X` 删本地分支**（顺序不能反：分支被 slot 检出时删不掉）→ 删任务**私有**非缓存生成物（`.tmp` 日志等；**slot 的 `server/target`/`client/build` 保温缓存与共享 `CARGO_TARGET_DIR` 严禁任务级清理**）→ 标记 slot 空闲。slot 本身不 remove 不 prune。旧流程/异常残留的一次性 worktree 由主干跑 `bash scripts/wt-janitor.sh` 巡检回收（仅 PR **MERGED** 且工作区干净、无非缓存 ignored（如 `.env`）、且无未合入 patch 的树才 `--apply` 自动收；squash 合入后远端 branch 删除时以 `git cherry origin/main` 判定 patch 等价；CLOSED/UNKNOWN/无 PR/脏树/含 `.env` 等一律交人工；脚本主动枚举 ignored，不依赖 `git worktree remove` 拒绝）——历史上残留 worktree + 缓存曾塞掉上百 G（2026-07-17 实测塞满 444G 盘）。远端分支不动（返工/merge 还要用），PR merge 后由 squash-merge 删或 `git push origin --delete`
- **claim 锁的释放也归主干**：PR merge 后核验远端 claim 分支确已删除（不依赖仓库自动删分支设置，没删就 `git push origin --delete bugfix/plan-X`）；PR close 未合并且确认放弃时，先核验无开放 PR、无在跑 subagent，再删 ref 让任务重新开放——锁不能靠"大概会自动清"悬着。**唯一 subagent 删除例外**：仅限该 subagent 本轮 create-ref 刚创建、PR 尚未创建、且删除前重新查询确认远端 ref SHA 仍严格等于本轮 `claim_sha` 的失败回滚；任一条件不满足即保留 ref 交主干，不得删除。**孤儿锁回收**：claim 成功但 PR 尚未创建时 subagent 异常退出/失联，主干确认无开放 PR、无存活 subagent、远端无需保留的提交后删 claim ref 重开任务；每轮补位时顺带巡检一遍孤儿锁。除上述严格失败回滚外，其余 claim ref 的释放、巡检与删除统一由主干负责。claim ref 的创建/删除是**锁运维**，是主干「不 push」禁令的唯一例外（该禁令约束的是提交/分支内容，不是锁 ref 生命周期管理）
- **review 返工也是主干的调度责任**：主干盯 in-flight PR 的 Kody 结果，出现修改意见时派**返工 subagent** 从 PR 分支进驻空闲 slot（原任务的进驻早已释放无妨，分支在远端；无空闲 slot 时返工排队并优先于新 skeleton 派发）。返工进驻也必须先用 `scripts/slot_registry.sh acquire` 获取 opaque `OWNER_TOKEN`，完成 detached/clean/ignored 与四方 SHA 对拍后，再用同一 token 通过唯一 `occupy` 门；失败按 token 化 `rollback`，成功后按 token 化 `release`/`freeze-blocked`，不得绕过 slot admission。返工序列（幂等，**不得重复 promotion / Finish Evidence 追加 / git mv 归档**）：修代码 → 按栈门禁 → fetch/merge 最新主线（带进变更则复验）→ 结论或证据变化时只**原地更新**已归档 plan 的 Finish Evidence → push 同一远端分支 → 等新 HEAD 的 e2e 与自动 review；发现问题或需要复审时发送 `@kody review --force`——review 意见永远有责任主体，不悬空

### Subagent（修复）流程

1. **Claim + 进驻常驻 slot**：subagent 是 claim ref 的**唯一创建主体**。分支名固定 `bugfix/<plan-basename>`，认领 = create-ref API 原子创建远端分支：`gh api repos/{owner}/{repo}/git/refs -f ref="refs/heads/bugfix/plan-X" -f sha="$(git rev-parse origin/main)"`——**201 = 认领到手**；**422 先甄别再判占用**（查响应体 / `git ls-remote` 确认同名 ref 确实存在才算被占、回报主干换任务；其他原因的 422 = 流程错误，上报诊断而不是换任务）。认领成功后 `git fetch origin bugfix/plan-X` 同步远端引用，再进驻主干分派的常驻 slot：先用 `out=$(bash scripts/slot_registry.sh acquire --slot slot-k --task <plan> --branch bugfix/plan-X --claim-sha <sha> --agent <id>)` 原子获取 reservation，并从仅本次 stdout 提取 `OWNER_TOKEN`（默认 status 不暴露；失败=换 slot/排队，禁止无 reservation checkout）→ 核验 detached + `git status --porcelain=v1 --untracked-files=all` 为空 + ignored 仅缓存白名单 → 本地分支不存在时 `git checkout -B bugfix/plan-X origin/bugfix/plan-X` 并执行带 `--agent <canonical-id> --owner-token "$owner_token"` 的 `mark-created-local --value true`，本地分支已存在则直接 `git checkout` 并核验 SHA==claim SHA（不一致转人工，**禁 `checkout -B` 覆盖残留提交**，`created_local_branch` 保持 false）+ 显式设 upstream，配置 upstream 后，只通过带 `--agent <id> --owner-token "$owner_token"` 的 `occupy` executable gate 进驻（由命令自己重验 canonical path、registered+locked、branch/HEAD/upstream/claim、dirty/untracked/ignored）；slot 不存在时主干先 `git worktree add --lock --detach` 一次性创建。**进驻失败回滚**：slot 内 detach（若已 checkout）+ `bash scripts/slot_registry.sh rollback --slot slot-k --task <plan> --agent <id> --owner-token "$owner_token"`，**仅当 stdout `DELETE_LOCAL_BRANCH=true`（本轮新建本地分支）才 `git branch -D`**；既有分支（含 SHA 冲突/BLOCKED 残留）一律保留并交人工。远端 claim ref 也只允许该 subagent 在「本轮 create-ref 刚创建、PR 尚未创建、且删除前重新查询确认远端 SHA 仍等于本轮 claim SHA」三项同时成立时回滚删除并核验不存在；否则保留 ref 交主干。slot 不 remove。
2. **Promotion**：`git mv docs/plans-skeleton/plan-X.md docs/plan-X.md`，单独中文 commit（本工作流内的 promotion 由 subagent 在自己分支内完成，是「骨架 → Active 人工流转」的授权例外）
3. **第一性原理验真**：不信 skeleton 的结论，自己读代码 / 写复现证明是不是真 bug
   - **真 bug** → 最小正确修复 + 最小契约测试锁住该 bug 的可观察行为，按小阶段中文 commit（每个 commit 带 `Model:` 署名 trailer，见「Commit 约定」）
   - **非 bug** → 在 plan 文档写「验证结论 + 证据」（docs-only commit），照常走后续归档 + PR
4. **本地门禁**：**按所触栈在对应目录跑，不跨栈乱调命令**——server：`scripts/build-token.sh cargo fmt --check && scripts/build-token.sh cargo clippy --all-targets -- -D warnings && scripts/build-token.sh cargo test`；client：`scripts/build-token.sh gradle test build`；agent/schema：对应包 `npm test`（schema src 改动先 `cd agent && npm run build -w @bong/schema`）；BongWorldGen：在独立仓库运行其 `pytest` 和生成器测试。跨栈修复 = 所有受影响栈都跑。管道尾必须取 `${PIPESTATUS[0]}`（`| tail` 吞退出码假绿）；测试失败绝不甩锅 pre-existing（见「测试诚实性」节）
5. **合并主线再验**：`git fetch origin && git merge origin/main`（fetch 必须紧邻 merge，防长跑 worktree 拿着陈旧远端引用）。merge 带进任何变更 → **重跑受影响栈完整门禁**（并行 PR 改同一结构体时 auto-merge 会叠出重复字段 E0062/E0415，只重编译不够）；产生冲突或触及修复相关文件 → 重新跑受影响栈门禁
6. **归档**：把 plan 各阶段状态更新为 `✅ YYYY-MM-DD` + 补 `## Finish Evidence`（字段按上文「Plan 文件结构」§3）——归档前置与三态流转契约一致（全部阶段 ✅ 且 Finish Evidence 齐），然后独立中文归档 commit `git mv docs/plan-X.md docs/finished_plans/plan-X.md`——非 bug 的验证结论同样归档，不给 origin/main 留僵尸 active plan
7. **Push + 开 PR**：`git push` 到 step 1 的 claim 分支并确认成功，`gh pr create --head bugfix/<plan-basename>`（中文标题 + body，两者都带完整 plan basename 供查重检索；body 末尾按「Commit 约定」注明执行模型与 reviewer 模型）。PR 有新提交/变动时默认由 Kody 自动 review；发现问题或需要复审时，再执行 `gh pr comment <PR> --body "@kody review --force"`。等 e2e 绿，回报主干闭环。**merge 不在本工作流内**——按「PR review gate」节走，由用户或后续会话收口；review 修改意见由主干派返工 subagent 接手（见上）

## Testing — 契约驱动的必要测试

**核心原则**：测试保护稳定、可观察且有真实回归风险的业务契约，不以测试数量、覆盖率、每个函数或每个 enum 变体都命中为目标。每个测试或表驱动测试组必须能回答“保护什么契约、避免什么风险”；回答不出来的测试应删除或改写。

- **必须保留的契约**：安全/权限、原子性/并发、真元守恒、具有不同外部结果的状态转换、跨进程或跨版本协议/schema、持久化兼容，以及已发生 bug 的最小回归。跨栈链路只有在消息、序列化、异步时序或副作用无法由单栈测试证明时才写 E2E。
- **硬编码值的边界**：packet ID、编码顺序、文件权限、版本化 type tag、领域物理常量和明确对外配置可以精确断言；优先引用生产常量。字段数量、私有字段顺序、默认构造细节、fixture 文案/地图名/演示 tick、扫描顺序和源码字符串不是契约，除非有明确外部消费者证明相反。
- **最小判别集**：每个行为等价类保留一个代表 case 加必要边界；多个 enum/input 走同一无分支路径时用代表 case 或表驱动合并，不做组合穷举。只有不同 variant 或 state transition 导致不同可观察结果时才分别测试。
- **测契约不测实现**：断言 IO、协议、副作用、持久化结果和 payload 结构；不要绑定私有字段、调用次数或中间步骤。等价重构不应让测试红。源码 grep、函数名、变量名或命令拼写断言不能替代行为测试。
- **mock 只覆盖被依赖的契约**：下游未实装时，mock 提供调用方需要的真实接口和行为；不为 mock 自身的所有内部实现分支写同构测试。真实实现接入后保留同一调用方契约。
- **失败信息带修复线索**：assert 写清期望的契约和失败原因；删除测试也必须在测试重构记录中说明其实现镜像性质或与保留测试的重复关系。

---

# Agent 行为硬约束

> 以下各节自原 `AGENTS.md` 并入（原文件是 oh-my-opencode 注入层，随多 harness 布局废弃）；`AGENTS.md` 现为指向本文件的 symlink，供按惯例读取 AGENTS.md 的 harness（Codex 等外部 orchestrator）共用同一份约束。

## 禁止动作

- 用户明确要求 `commit` / `push` / `开 PR` / `gh pr create` 时，视为已授权普通提交、普通推送和 PR 创建，**无需二次确认**
- 仍需明确确认：`git push --force`、`git reset --hard`、`git commit --amend`、交互式 rebase、批量删除/移动文件、依赖版本或生产配置改动
- 严禁 `--no-verify`、`--no-gpg-sign`、`-c commit.gpgsign=false`
- 不绕过 "Java 17 for Fabric" 约定；不要跨栈调命令（server 里不跑 npm、agent 里不跑 cargo）
- 不改 `.gitignore`、`package.json`、`Cargo.toml` 的依赖版本（除非当前 plan 明确要求）
- 不向 `docs/worldview.md` 回写——世界观锚点只在核心 canon 改动时人工修，且必须单独 PR 人工 review
- 不向 `docs/library/` 主动回写（图书馆域走 `/write-book` / `/review-book` 专门流程，plan 流水线不跨界）
- **`git stash push` 无对等 `git stash pop`**：任何 auto-stash 的流程，完成时必须把自己产生的 WIP stash pop 回来；不得在主仓库留下 `WIP before inspecting ...` 孤儿 stash（历史教训：曾 stash + `reset --hard` 主仓库但不 pop，用户 worktree 改动凭空"消失"直到从 stash 捞出）

## Commit 约定

- commit message **中文**，匹配仓库近 30 提交风格；每个逻辑单元一个 atomic commit，不堆积巨型 commit
- 归档 commit 形如：`归档 plan-<name>：<一句话总结>`
- **模型署名（供后续统计，必填）**：agent 产出的每个 commit 末尾必须带 trailer 注明**真实执行模型**：`Model: <精确模型 id>`（如 `claude-fable-5` / `claude-opus-4-8` / `gpt-5.6-sol-xhigh`），`Co-Authored-By` 照旧保留。PR body 末尾同样注明主导模型及参与模型（reviewer 用了不同模型也逐个列出）。不许漏署，不许写泛称 "AI" / "agent"。统计入口：`git log --format='%(trailers:key=Model,valueonly)'`

## 世界观正典硬锚（写代码/schema/命名前先对，别凭"修仙常识"）

唯一权威 `docs/worldview.md`。下面是**最常被违反**的几条，违反 = review 直接打回：

- **六境界**（worldview.md §三 L67-L72，顺序固定；worldview.md §三 L63 明禁旧称）：**醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚**。严禁上古称呼：练气 / 筑基 / 金丹 / 元婴。
- **命名禁词**（worldview.md §三 L63 的命名原则落地速查）：末法时代禁用 玄/陨/星/仙/太/古；优选衰败素朴意象 残/碎/锈/杂/粗/髓/朴/枯。例外：已入世俗医药的矿名（丹砂/朱砂/雄黄）OK。
- **经济**：唯一真货币 = **骨币**（异变兽骨+阵法锁真元）；矿物=交易筹码非货币；灵石=劣质衰变物+燃料；金银=废土。
- **zone 命名**：写新 zone 前查 worldview.md §十三 L1253-L1260 区域表和 `server/zones.json` 既有 ID（已立：spawn / 青云残峰 / 血谷 / north_wastes / lingquan_marsh 等）。
- 引用格式统一 `worldview.md §X L<line>`，便于回查。

## 真元/灵气守恒律（最高优先级硬约束，吞真元 = 阻塞合并）

全服灵气总量 `SPIRIT_QI_TOTAL` 恒定（const 当前 100.0；**测试断言取 const 引用，不写字面 100**）。所有真元/灵气流动**必须**走 `qi_physics::ledger::QiTransfer { from, to, amount, reason }`。

**红旗（出现就停下重设计）**：
- `cultivation.qi_current += X`（无对应 zone 减）、`zone.spirit_qi -= Y`（无对应玩家增）、容器/衰变把真元"凭空消失"不归还 zone、招式释放只扣攻方不写入环境 —— 全是守恒律红旗。
- 离屏/抽象战斗死亡：携 `qi_current > 0` 的快照直接 `store.remove` 丢弃、或只 `emit QiTransfer` 事件却**无 system 消费**应用到 `WorldQiAccount` = 吞真元。离屏战死必须走 `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)`。
- **自定真元物理常数/公式**：新模块出现 `*_DECAY*` / `*_DRAIN*` / `*_ATTEN*` / `*_HALF_LIFE*` / `RHO` / `BETA` / 形如 `0.0X_f64` 的"看起来像衰减率"常数 / `fn *_decay()` → **必查 `qi_physics`**（plan-qi-physics-v1）。已存在就调用；不存在就**先扩 `qi_physics::constants` 再 import**，**禁止 plan 自己写一份**。
- 唯一允许的"系统外流"= 天道每时代衰减 1-3%（`qi_physics::tiandao::era_decay_step`，常数 `QI_TIANDAO_DECAY_PER_ERA_MIN/MAX`）。注意它**不是凭空蒸发**：`WorldQiBudget::apply_era_decay` 把衰减量挪进被追踪的沉降槽 `era_decay_accum`，不变式 `current_total + era_decay_accum == initial_total` 恒成立。守恒口径用 `qi_physics::ledger::assert_conservation(before, after, era_decay)` 断言。
- 释放走 `qi_release_to_zone`，吸收走 `qi_excretion`，坍缩渊塌缩走 `collapse_redistribute_qi`（中转站不是终点）。

> 完整孤岛红旗清单见 `docs/CLAUDE.md §四`——碰 gameplay/qi plan 时先读一遍。

## 招式/技能 A/V 差异化（战斗/skill 类 plan 的红线）

任何 skill / cast / 招式 / 主动能力落地，**必须**携带**每招独立可辨**的：① animation ② particle/VFX ③ SFX ④ HUD 反馈 ⑤ hotbar/SkillBar 槽位 PNG icon。

- "只动 server 算子先 ship、客户端 P 后补" = **红旗**——招式没视觉就不算 P0/P1 完成。仅 server 算子/仅 schema enum 不算"实装"。
- skill plan 的 `§N 客户端动画/VFX/SFX` 段必须**表格化列出每招独立的 animation+粒子+音效+HUD+icon 名**（基线范本：`docs/finished_plans/plan-yidao-v1.md §5`）。
- 验收末阶段必须含**视觉/听觉差异化回归 + icon 显示回归**（玩家能从远处分辨"对面在用 X 不是 Y"）。
- icon 资产：新招 PNG 走 `/gen-image item`（`scripts/images/gen.py`），路径 `client/src/main/resources/assets/bong/textures/skill/<style>/<skill_id>.png`（16×16/32×32，化虚级 `<skill_id>_void.png` 高分辨率+染色描边）；server `SkillDef.icon_id` → schema 双端镜像 → client `SkillIconRegistry` 查图。
- **当前 harness 跑不了 `/gen-image` 时**：写好 server/schema/client 接线 + 占位资源 + 在该 TODO 标 `[BLOCKED: 需 /gen-image 生成 <清单>]`，继续其它 TODO，不要画手绘糊弄、也不要跳过接线。

## 视觉资产纪律（NBT 建筑 / layout / 模型 / 贴图）

- **3 轮打磨 + `<PROMISE>` 担保**：NBT 建筑、worldgen layout 摆位、复杂模型、视觉资产**禁止一把 commit**。Round 1 first cut → **Round 2 人工闸门** → Round 3 终轮，commit message 标 `(round N/3)`；终轮 commit 末尾写 `<PROMISE>...已 3 轮打磨...已检查[...]...仍存局限[...]</PROMISE>` 块（**拼写是 PROMISE 不是 PROMIS**）。纯 Rust/TS 逻辑 TODO 不适用。
- **Round 2 是人工闸门，不是模型自评**：固定产出**一张给人看的接触表**，然后**停下等人一句话**再动 round 3。bbmodel 类走 `bbmodel-contact-sheet <模型> --gates <生成器模块> --prev <上一轮>`，表里必须有四样：① 六个**诚实命名**的视角（标签写出实际照到的轴面）② 上一轮的**同一取景**对比 ③ manifest 点名结果 ④ 门禁的差分自证结果。
  - 改掉「自评」的理由是两次实测，**都恰好发生在自评这一步**：`yaw=180` 名义叫 FRONT 实渲 −z 面，害人在错的视角上连试三个亮度阈值去找一个本就不该出现的骨扣（几何/UV/材质从头到尾都是对的）；小草包前两轮**整件漏掉背带**（参考图里占比仅次于包身）而七道数值门全绿 —— 有没有背带根本不在任何一道门的问题域里。人看图三十秒能问出「背带呢」，模型跑四十分钟数值门也问不出来。
  - **任何「让模型自己判断像不像参考图」的设计都是错的** —— 自己出题自己判卷。特征清单 `modelScript/manifests/<Asset>.manifest.toml` **必须人写**，点名器只负责核对。
- **「自检全绿」在做差分注入之前，信息量是零**：判据本身会假绿而模型不会怀疑它 —— 某版穿模判据白名单写反，**坏版本和修好的版本都报 17 处**，零区分力却两边都「有输出」。所以 `bbmodel_maker.gates.gatekit` 每道门旁边就是它的注入器（动画侧同理见 `animgate`），跑 `--self-test` 先注入缺陷再跑，报不出违例的门直接算失效。新写判据先问一句：**把它该抓的东西造出来，它报得出来吗？**
- **复杂模型分部件做**：拆 `part_base()` / `part_body()` / ... 函数，逐件单独预览，最后 `all_cubes()` 拼接（别整件一把梭埋掉单件缺陷）。bbmodel 真长相用 `bbmodel-render <模型>` 看，别只信平涂示意图。
- **item icon 批量出**：新增 ItemTemplate 必配 icon，走 `/gen-image item`（批量、不需多轮）。跑不了 `/gen-image` 的 harness 标 `[BLOCKED: 需 /gen-image]`。

## 架构硬约束（entity / 动画）

- **禁止 vanilla MC entity hack**：不准用 armor stand / invisible mob 充当碰撞箱或交互载体。Bong 的 entity 是 **Marker + 自定义渲染**，交互走 **C2S 请求**（client 注册 IntentHandler / InteractKeyRouter / 右键准星检测 → C2S → server），不走 vanilla InteractEntityEvent。范本：NPC 的 `NpcEngagementIntentHandler → NpcInspectRequest`。绝不切到有碰撞箱的 EntityBundle。
- **PlayerAnimator 四大库坑**（写动画必读，不看源码猜不到）：
  1. **循环动画单帧衰减**：`isLooped=true` 时只在 tick 0 放关键帧的 axis 会被插值回 `defaultValue`——每个用到的 axis 必须在 `endTick` 补一个同值 keyframe。
  2. **MC 无 IK**：`leg.pitch > ~35°` 腿腹断连。大 pitch 用 `bend`（小腿后折）承担，pitch 控在 40° 内；别给 leg 加 z 偏移（更糟）。
  3. **`body.*` 走 MatrixStack 不是 ModelPart**：整体位移/旋转（含头发盔甲手持物）。要"上半身扭下盘不动"用 `torso.yaw`，不用 `body.yaw`。
  4. **`bend` 需 bendy-lib 否则静默 no-op**：已配 `bendy-lib 4.0.0`（MC 1.20.1 唯一可用版本）于 client depends，别动版本。
  迭代姿态用 headless 工具 `client/tools/render_animation.py`（出三视图 PNG，免 build jar + runClient）。完整约定见 `docs/player-animation-conventions.md`。

## 测试诚实性 + 构建/CI 坑

- **绝不把自己引入的失败甩锅 "pre-existing"**：上一已 merge 阶段是 `0 failed`、本阶段突现 N failed = 本 PR 引入；共同 signature（registry/asset 加载、schema parse、template exist）= 单点根因（一个坏 config 连锁红一片）。
- **`ItemCategory` 合法集**（`server/src/inventory/mod.rs`）：Pill / Herb / Scroll / Misc / Weapon / Armor / Tool / Treasure / RecipeFragment / RecipeHint / BoneCoin / Container —— **无 Material**。炼丹材料用 `Misc`（用 `material` 会让整个 item registry 加载失败 → 连锁红几十个测试）。
- **schema 改了必重建 dist**：`agent/packages/tiandao` 经 `@bong/schema` 引用的是构建产物 `dist/`，不是 src。改了 `agent/packages/schema/src/*.ts`（新增 export/改 schema）后必须 `cd agent && npm run build -w @bong/schema`，否则 agent 启动崩 `SyntaxError: does not provide an export named 'X'`。
- **headless/CI 启服必设 `export BONG_SKIP_SKIN_PREFETCH=1`**：否则 `maintain_skin_pool` 因缺 `MINESKIN_API_KEY` panic（`src/skin/pool.rs`）。配 dummy key 没用（超时再 panic）。
- **真集成 gate = `e2e`**（`bash scripts/smoke-test-e2e.sh` / `e2e` CI check），不是 snapshot；判 worldgen/server 改动看 e2e + 单测最可靠。`main` 未设保护，多数 check 非 required。

## PR review gate

- **gate 只看 Kody**，绝不把 Codex connector 的限流噪音当成代码结论。Kody 按当前配置做 bug、性能、安全和业务逻辑 review，是本仓库唯一在跑的 LLM 审查器。
- **默认自动 review**：PR 创建时以及 push 新提交后由 Kody 自动 review。不要发送 `/review`、`/review-next` 或 `@kody start-review`；发现问题或需要针对最新 HEAD 复审时，才在 PR 根评论发 `@kody review --force`。
- **判审查归属必须核 SHA，不能只看时间戳**：旧 HEAD 的延迟评论会落在新 push 之后。认行内评论的 `original_commit_id`——**`commit_id` 会被 GitHub 改写成当前 HEAD，用它必得假阳性**（#2058 实测 6 条里 4 条被改写）。同时按 `--paginate` 拉取完整评论，并优先确认评论对应当前 HEAD；Kody 的 clean 总结可能没有 SHA，必要时用 `@kody review --force` 获取当前 HEAD 的新结论。详见 `docs/CLAUDE.md §6.5`。
- **CodeRabbit 不在默认等待范围**：`.coderabbit.yaml` 已关闭自动触发；确实需要第二双眼睛时才评论 `@coderabbitai review` 按需拉起。也不要用 `@pi`/`@hive`/`@claude` mention 陌生账号。
- 等待用 `ScheduleWakeup delaySeconds=1200`（~20 min/回合，最多 3 回合卡死才停交人工），禁止 sleep loop / busy-poll。修完 review 意见要重新等 re-review，不自判"应该过了"（完整协议见 `docs/CLAUDE.md §6.5`）。
