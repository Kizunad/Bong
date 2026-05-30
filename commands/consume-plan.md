---
description: 开 worktree → Workflow 多 agent 实施 docs/plan-<name>.md（设计收口→逐阶段实施→对抗审查，全程在 worktree 内）→ PR → 查 merge conflict → 等 CI+codex review → 自行判定 → 自动 merge → 清理。全自动；测试/CI 失败允许有限修复（≤2 轮）、review 意见自行判断采纳，仅严重设计问题/反复修不过才交人工。无 Workflow 工具的 harness（如 Codex）退化为线性实施。
argument-hint: <plan-name>
---

# consume-plan $ARGUMENTS

消费 `docs/plan-$ARGUMENTS.md`：**step 3 实施由 Workflow 多 agent 编排**（设计收口→逐阶段实施→对抗审查，全程在 worktree 内），其余步骤线性。**全自动到 merge**——PR 创建后**独立发 comment** mention `@codex` 触发 review（写在 PR body 里不生效，bot 只捕获 issue comment / review comment），step 6 等 Codex 给评论后 step 7 自行判定（严重必修、nit 忽略并写理由），无严重问题即 step 8 自动 squash merge。

实施期测试/CI 失败允许**有限次本地修复（≤2 轮）**；review 意见自行判断采纳；merge conflict 先尝试 rebase（≤2 轮），拿不准交人工。不自动跳过失败 TODO。

> **Workflow 是 Claude Code 专有工具**。当前 harness 没有 Workflow 工具（如 Codex）时，step 3 退化为 §3.5 线性逐 TODO 实施，其余不变。

---

## 核心约束（执行前必读）

1. **所有 git / commit / push / test / gh 命令必须在 worktree 内执行**。主仓库根目录（`/home/kiz/Code/Bong`）HEAD 指向 `main`，在那里做任何写操作等同于直接写 main 分支——**严禁**。
2. 你的 bash 调用之间 cwd **不保证持久**。每个 bash block 开头都必须 `cd "$WT_ABS"`，或用 `git -C "$WT_ABS" ...` 显式指定。
3. 只有 step 9 收尾清理才允许 `cd "$REPO_ROOT"` 回主工作区。中途**严禁** `cd ..` / `cd -` / `cd /home/kiz/Code/Bong` 离开 worktree。
4. 严禁 `--no-verify` / `--no-gpg-sign` / `git reset --hard` / `git push --force` 等绕过或破坏操作。**唯一例外**：step 4.2 rebase 解决冲突后允许 `git push --force-with-lease origin "$BRANCH"`（远端 head 校验，不会覆盖他人 commit）。
5. **`docs/` 写权限严格限于本次消费的 plan**：仅允许在 `docs/plan-$PLAN.md` 末尾追加 `## Finish Evidence` 章节，并最终 `git mv` 入 `docs/finished_plans/`（详见 step 3 末尾"全 P 完成后"）。其他 `docs/` 文件 / `CLAUDE.md` / `worldview.md` 严禁自动改——遇到必须改的情况停下交人工。
6. 严禁用注释掉测试 / `#[ignore]` / skip case / 改断言数值等方式"让测试过"。
7. **Workflow 编排约束（step 3）**：Workflow 起的 subagent **继承会话 cwd = 主仓库根 `$REPO_ROOT`，不会自动进 `$WT_ABS`**。因此 ① 必须把 `$WT_ABS` 绝对路径经 `args` 传入 workflow，并在**每个 agent prompt** 里强制"只在 `$WT_ABS` 内操作（`cd "$WT_ABS"` 或 `git -C "$WT_ABS"`），严禁碰主 checkout / `cd` 离开 / 动 main"；② **禁用** agent 的 `isolation:'worktree'`（会给每个 agent 各自开新 worktree，与本流程单一 `$WT_ABS` 冲突）；③ 实施 agent 同样严禁 `push` / 开 PR / `merge`（归主流程 step 4+），不写 plan 文档正文（Finish Evidence 归主流程）。

---

## 1. 前置校验

```bash
PLAN="$ARGUMENTS"
[ -n "$PLAN" ] || { echo "❌ 用法：/consume-plan <plan-name>（不含 plan- 前缀和 .md 后缀）"; exit 1; }

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

[ -f "docs/plan-$PLAN.md" ] || { echo "❌ docs/plan-$PLAN.md 不存在"; exit 1; }
# 骨架 / 归档拒绝
if git ls-files "docs/plans-skeleton/plan-$PLAN.md" "docs/finished_plans/plan-$PLAN.md" 2>/dev/null | grep -q .; then
  echo "❌ 骨架或已归档 plan 拒绝消费"
  exit 1
fi
git fetch origin main
```

## 2. 开 worktree（幂等，固定绝对路径）

```bash
REPO_ROOT="$(git rev-parse --show-toplevel)"
WT_ABS="$REPO_ROOT/.worktree/plan-$PLAN"
BRANCH="auto/plan-$PLAN"

if [ -d "$WT_ABS" ]; then
  echo "[info] worktree 已存在，复用 $WT_ABS"
else
  git -C "$REPO_ROOT" worktree add "$WT_ABS" -b "$BRANCH" origin/main
fi

cd "$WT_ABS"
# 双重校验：cwd 和分支
[ "$(pwd -P)" = "$(cd "$WT_ABS" && pwd -P)" ] || { echo "❌ cwd 不在 $WT_ABS"; exit 1; }
[ "$(git rev-parse --abbrev-ref HEAD)" = "$BRANCH" ] || { echo "❌ 分支错误，应为 $BRANCH"; exit 1; }
echo "[ok] cwd=$WT_ABS branch=$BRANCH"
```

**`$WT_ABS` 和 `$BRANCH` 从这里起为固定值**。后续任一 bash block 都先 `cd "$WT_ABS"` 并在必要时重算这两个变量。

## 3. 实施 plan（Workflow 多 agent 编排）

实施交给 **Workflow 工具**编排，三段流水：**设计收口（并行只读）→ 逐阶段实施（按 P 依赖顺序，串行）→ 对抗审查（并行只读）**，全程在 `$WT_ABS` 内。Workflow 是 slash command 显式授权的合法入口（满足 Workflow 工具的 opt-in 条件）。

> 当前 harness 无 Workflow 工具（如 Codex）→ 跳到 **§3.5 线性 fallback**，其余步骤不变。

### 3.1 准备 workflow 输入

```bash
cd "$WT_ABS"
# 从 plan 头部"阶段总览"解析 P 列表（按依赖顺序）；无显式分阶段的简单 plan 用单一伪阶段 "ALL"
PHASES=$(grep -oE '\| (P[0-9]+) \|' "docs/plan-$PLAN.md" | grep -oE 'P[0-9]+' | sort -u | tr '\n' ' ')
[ -n "$PHASES" ] || PHASES="ALL"
echo "[workflow-in] WT_ABS=$WT_ABS PLAN=$PLAN PHASES=$PHASES"
```

### 3.2 调 Workflow（下方模板，按 plan 实际阶段裁剪）

用 Workflow 工具传 `args: { wtAbs: "<$WT_ABS 绝对路径>", plan: "$PLAN", phases: [<P 列表>] }`。**每个 agent prompt 都内联"只在 wtAbs 内操作"约束（核心约束 #7）**。

```js
export const meta = {
  name: 'consume-plan-impl',
  description: 'Workflow 实施 docs/plan-<PLAN>：设计收口→逐阶段实施→对抗审查（全程 worktree 内）',
  phases: [
    { title: 'Design', detail: '并行只读：收口开放问题 + 接入面 + 逐阶段工作项' },
    { title: 'Implement', detail: '按 P 依赖顺序逐阶段实施 + atomic commit + 测试矩阵' },
    { title: 'Verify', detail: '并行对抗审查累积 diff：契约/饱和测试/CLAUDE.md/回归' },
  ],
}
const WT = args.wtAbs, PLAN = args.plan, PHASES = args.phases
// 每个 agent 都带这条硬约束（核心约束 #7）
const GUARD = `**只在 ${WT} 内操作**：bash 以 \`cd ${WT}\` 开头或用 \`git -C ${WT}\`；严禁 cd 离开 / 碰主 checkout / 动 main / push / 开 PR / merge / --no-verify / #[ignore] / 注释测试 / 改断言蒙混 / 改 plan 范围外代码。不写 plan 文档正文。`
const PLAN_DESIGN = { type:'object', required:['summary','decisions','workItems'], properties:{
  summary:{type:'string'},
  decisions:{type:'array',items:{type:'object',required:['question','decision','landing'],properties:{question:{type:'string'},decision:{type:'string'},landing:{type:'string'}}}},
  workItems:{type:'array',items:{type:'object',required:['phase','what','files'],properties:{phase:{type:'string'},what:{type:'string'},files:{type:'array',items:{type:'string'}}}}} }}
const IMPL = { type:'object', required:['phase','commits','testsPassed','testsFailed','fmtClippyPass','blocked','summary'], properties:{
  phase:{type:'string'},commits:{type:'array',items:{type:'string'}},testsPassed:{type:'integer'},testsFailed:{type:'integer'},fmtClippyPass:{type:'boolean'},blocked:{type:'string',description:'空=通过；非空=失败命令+错误原文'},summary:{type:'string'} }}
const VERDICT = { type:'object', required:['dimension','passed','issues'], properties:{
  dimension:{type:'string'},passed:{type:'boolean'},
  issues:{type:'array',items:{type:'object',required:['severity','location','desc'],properties:{severity:{type:'string',enum:['blocker','major','minor']},location:{type:'string'},desc:{type:'string'}}}} }}

phase('Design')
const design = await agent(`读 ${WT}/docs/plan-${PLAN}.md + 相关代码（只读，不改不 commit）。${GUARD}
产出：① 收口 plan 所有"§N 开放问题"——每条给决议 + file:line 落点（决议只在本次 context 用，**不写回 plan 文档**）；② 接入面（进料/出料/共享类型/跨仓库契约 symbol）；③ 把实施拆成按 P 依赖排序的 workItems（每项标 phase + 改哪些文件）。按 schema 输出。`,
  { phase:'Design', schema: PLAN_DESIGN })

phase('Implement')
// 串行：同一 worktree 不能并行写；后阶段依赖前阶段
const implResults = []
for (const ph of PHASES) {
  const r = await agent(`你在 ${WT} 实施 docs/plan-${PLAN}.md 的 ${ph} 阶段。设计决议（已收口）：${JSON.stringify(design)}
${GUARD}
- 按 plan ${ph} 交付物实施，每个逻辑单元一个 atomic commit（fix/feat/refactor 中文前缀，照 CLAUDE.md）。
- **饱和测试**（CLAUDE.md「饱和化测试」）：happy / 边界(empty/max/off-by-one) / 错误分支 / 状态转换都覆盖；测契约不测实现；plan 点名的测试必须实现。
- 每次 commit 后在 ${WT} 内跑对应子项目测试矩阵（见下），必须全绿。
- 失败：本阶段 patch 引入的明显错误自修（≤2 个独立 fix commit），仍不过则把"失败命令 + 错误原文"写进 blocked 字段并停（不跳过、不继续后阶段）。
按 schema 输出（blocked 空=通过）。ultrathink`,
    { phase:'Implement', schema: IMPL })
  implResults.push(r)
  if (r.blocked) break   // 某阶段卡住即停，保留 worktree commit
}

phase('Verify')
let verdicts = []
if (!implResults.some(r => r.blocked)) {
  const DIMS = [
    '契约与回归：本 PR 是否破坏外部可观察行为/IO/协议/现有运行时？散布/序列化/状态机有无回归？',
    '饱和测试完整性：happy/边界/错误/状态转换有无漏 case？有无 #[ignore]/注释测试/改断言蒙混？',
    'plan 目标对齐 + CLAUDE.md/worldview/qi_physics 守恒合规',
  ]
  verdicts = (await parallel(DIMS.map(d => () =>
    agent(`对抗审查 ${WT} 内本次累积 diff（\`git -C ${WT} diff origin/main...HEAD\`）。只读，不改不 commit。${GUARD}
维度：${d}。**尽力证伪**——默认有问题，找不到才放行。按 schema 给 verdict + issues(blocker/major/minor + location + desc)。`,
      { phase:'Verify', schema: VERDICT })))).filter(Boolean)
}

return { design, implResults, verdicts, blocked: (implResults.find(r => r.blocked) || {}).blocked || null }
```

**测试矩阵**（实施 agent 与 §3.5 fallback 共用；agent 把 `$WT_ABS` 换成 `${WT}`）：

```bash
cd "$WT_ABS"
# Rust（server/ 改动）：
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
# Java（client/ 改动）：
cd client && ./gradlew test build
# TypeScript（agent/ 改动）：
cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)
# Python 改动：ruff PostToolUse hook 会自动格式化
```

### 3.3 读 workflow 结果

- `blocked` 非空（某阶段实施卡住）→ 按"失败统一行为"停：保留 `$WT_ABS` 内 commit，输出失败命令 + 原文 + `$WT_ABS`，交人工。**不**继续后续阶段、**不** `cd` 离开。
- `verdicts` 含 `blocker` / `major` → 在 `$WT_ABS` 内修复（可再起一个 fix agent，或主流程直接改）+ 重跑对应测试矩阵 + 重新审查该维度；**≤2 轮**仍不过交人工。修复范围严格限于审查指向的具体问题。
- `verdicts` 仅 `minor`（nit/主观）→ 记录不强改，最终输出里说明。
- 全过 → 进 §3.4。

### 3.4 全 P 完成后：写 Finish Evidence + 归档（提 PR 前最后一步）

实施 + 审查全过后，**提 PR 之前**由**主流程**（非 workflow agent）按 workflow 返回的 `implResults` / `design` 写 evidence（docs 写权限仅限于此，见核心约束 #5）。所有 P0/P1/§N 都通过后：

1. 在 `$WT_ABS/docs/plan-$PLAN.md` 末尾追加 `## Finish Evidence` 章节，至少包含：
   - **落地清单**：每个 P 对应的真实模块/文件路径
   - **关键 commit**：本 worktree 内的实施 commit hash + 日期 + 消息（取自 `implResults[].commits`）
   - **测试结果**：跑过的命令 + 数量（如 `cargo test cultivation:: → 94 passed`）
   - **跨仓库核验**：server / agent / client 各命中的 symbol（plan 跨仓库时）
   - **遗留 / 后续**：未在本 plan 范围、依赖其他 plan 的待办（若有）

2. `git mv` 入归档目录并 atomic commit：

   ```bash
   cd "$WT_ABS"
   git mv "docs/plan-$PLAN.md" "docs/finished_plans/plan-$PLAN.md"
   git add "docs/finished_plans/plan-$PLAN.md"
   git commit -m "docs(plan-$PLAN): finish evidence 并归档至 finished_plans/"
   ```

3. 再跑一次对应子项目测试，确认 mv 没破坏路径引用（README / scripts / 其他文档不应硬编码旧 `docs/plan-$PLAN.md` 路径）

PR 内已包含归档动作——merge 后 plan 自动就在 `finished_plans/`，**不另开 PR**。

**review 阶段若 step 7 修改了实施**：相应更新 evidence（已 mv 后路径下的文件 `docs/finished_plans/plan-$PLAN.md`），保持文档与代码一致。

### 3.5 Fallback：无 Workflow 工具时线性实施

按 `docs/plan-$PLAN.md`（source of truth）的 P0/P1/§N 顺序推进，每个 TODO / 逻辑单元一个 **atomic commit**（fix/feat/refactor 中文前缀），每次 commit 后在 `$WT_ABS` 内跑上方**测试矩阵**全绿才进下一个 TODO。**饱和测试**要求同 §3.2 实施 agent。

测试/编译失败处理（有限修复循环，≤2 轮）：

1. **判归属**：本 TODO patch 引入（typo / 漏 import / 小 bug / 漏 `?` / borrow）→ 自修；plan 本身问题 / 需扩大改动面 → 停；明显 flaky → 重跑一次仍挂则停。
2. **自修**：最多 2 次独立 atomic fix commit（如 `fix(test): 补 XX import`），每次重跑对应测试。2 次仍不过 → 停。
3. **停**：保留 `$WT_ABS` 内所有 commit 和未提交改动，输出失败命令 + 错误原文 + `$WT_ABS`，**不继续 / 不跳过 / 不 `cd` 离开**，交人工。

修复范围严格限于"当前 TODO patch 引入的明显错误"——禁止顺手修其他模块、重构、改 plan 范围外代码。完成后同样走 §3.4。

## 4. 提 PR（不 auto-merge）

```bash
cd "$WT_ABS"
git push -u origin "$BRANCH"

PR_URL=$(gh pr create \
  --title "plan-$PLAN: <一句话摘要>" \
  --body "$(cat <<EOF
自动消费 \`docs/plan-$PLAN.md\`。

## 实施摘要
<按 plan 的 §/P 列主要改动>

## 本地测试
<粘跑过的测试命令 + 通过标记>

---

**有阻断意见请加前缀 \`BLOCKING:\` 或选 Request changes，否则按下方规则自动处理。**（review 触发 mention 见 PR 评论区独立 comment）

<details>
<summary>📜 自动 merge 协议（点击展开）</summary>

本 PR 由 \`/consume-plan\` 全自动消费产出。CI 全绿 + Codex reviewer 至少 1 条评论后，主流程会**自行判定**意见严重性并自动 squash merge：

- **严重**（bug / 安全 / 与 plan 目标矛盾 / 违反 \`CLAUDE.md\` / \`worldview.md\` / 命中 \`BLOCKING:\` 或 \`CHANGES_REQUESTED\`）→ 必修，CI 重绿后再 merge
- **中等**（明确质量问题但不影响功能：错误处理缺失、命名误导、文档与代码不符）→ 自行决定修或不修，不修会在 PR 内回复理由
- **轻微**（nit / style / 主观偏好 / "可以考虑"语气）→ 默认不采纳，merge 前统一回一条说明

reviewer 缺席（30 min 内无反馈）会在最终输出标注；Codex 缺席则停交人工不自动 merge。

</details>

🤖 Generated by /consume-plan
EOF
)")
PR_NUM=$(echo "$PR_URL" | grep -oE '[0-9]+$')
echo "PR: $PR_URL (#$PR_NUM)"

# 独立 comment 触发 codex review
# —— 写在 PR body 里 bot 不会捕获，必须以独立 issue comment 形式 mention 才生效。
# 正文极简，让 bot 抓 mention 即可。
gh pr comment "$PR_NUM" --body "@codex 请 review 这个 PR。"
```

**不加 `--auto`**——主流程要先看 review 评论再 merge（step 6 → 7 → 8 顺序），不能让 GitHub auto-merge 抢跑过 review 判定。

### 4.1 检查 merge conflict

PR 创建后**立即**查 mergeable 状态：

```bash
cd "$WT_ABS"
# GitHub 异步计算 mergeable，最多等 30 秒
MERGE_STATUS="UNKNOWN"
for i in 1 2 3 4 5 6; do
  MERGE_STATUS=$(gh pr view "$PR_NUM" --json mergeable --jq '.mergeable')
  [ "$MERGE_STATUS" != "UNKNOWN" ] && break
  sleep 5
done
echo "[mergeable] $MERGE_STATUS"
```

- `MERGEABLE` → 进 step 5
- `UNKNOWN`（30s 仍未算出）→ 不阻塞，进 step 5（CI / merge 阶段会再次校验）
- `CONFLICTING` → 走 4.2 自行 rebase 解决

### 4.2 自行 rebase 解决冲突（≤2 轮）

冲突时不一律交人工——agent 在 plan 上下文里，通常足以判断"main 改了什么、本 PR 改了什么、应该如何合并"。先尝试 rebase，能解决就解决，拿不准再停。

```bash
cd "$WT_ABS"
git fetch origin main
git rebase origin/main
```

**Case A：rebase 干净**（git 自动 3-way merge 成功，无冲突标记）

1. 跑对应子项目测试（按 step 3 测试矩阵）
2. 通过 → `git push --force-with-lease origin "$BRANCH"` → 回 step 5
   - 用 `--force-with-lease` 不用 `--force`：仅当远端 head 仍是上次 push 时的 head 才推，避免覆盖他人新 commit
3. 测试不过 → 走 case B 第 5 步

**Case B：rebase 中断**（出现 `<<<<<<<`/`=======`/`>>>>>>>` 冲突标记）

1. `git status` 列出所有冲突文件
2. 对每个冲突文件做语义合并：
   - `git log --oneline ..origin/main -- <file>` 看 main 改了什么
   - `git log --oneline origin/main..HEAD -- <file>` 看本 plan 改了什么
   - 判断原则：
     - 双方改同一函数 / 同一字段 / 同一行：合并语义，保留两边逻辑（典型如 imports、enum variant 列表、struct 字段、模块清单）
     - 一方删一方改：保留改的那边
     - 双方都加（同一区域）：拼接，注意去重和顺序
     - **拿不准 / 双方逻辑互斥需要业务决策 / 涉及 plan 范围外的代码** → 走第 5 步
3. 编辑文件去掉 `<<<<<<<`/`=======`/`>>>>>>>` 标记，`git add <file>`
4. `git rebase --continue`，可能还有下一个冲突点 → 回到 1
5. **任一步拿不准就 abort 停**：
   ```bash
   git rebase --abort
   echo "❌ rebase 冲突需要业务判断，停交人工"
   gh pr view "$PR_NUM" --json url,mergeStateStatus
   echo "WT: $WT_ABS"
   exit 1
   ```
6. rebase 成功后必须跑对应子项目测试（同 step 3），全绿才能 `git push --force-with-lease origin "$BRANCH"` → 回 step 5

**最多 2 轮**：第 1 轮 abort 后允许再尝试 1 次（例如先详读 main 上的相关 commit 重新评估）。第 2 轮仍卡住 → 停交人工。

修复范围严格限于"解决 rebase 冲突"——禁止顺手重构、改 plan 范围外代码、删 main 上别人新写的逻辑。

## 5. 等 CI 跑完

```bash
cd "$WT_ABS"
gh pr checks "$PR_NUM" --watch --fail-fast
```

- exit 0 = 全绿 → 进入 step 6
- 非 0 = 有 check 挂了 → 进入 CI 失败修复

### CI 失败修复策略（≤2 轮）

1. `cd "$WT_ABS" && gh run view --log-failed` 拉失败日志
2. **在 `$WT_ABS` 内复现**失败 step 的命令（通常是 `cargo clippy` / `./gradlew test` / `npm test`）
3. 本地能复现 → 本地修 + atomic fix commit + `cd "$WT_ABS" && git push` → 回到 step 5 重等
   - 最多 **2 轮修复**（首次失败后最多再推 2 次）
   - 每轮修复必须先在本地测试通过才 push
   - 修复范围同 step 3：限于引起 CI 失败的 patch 本身
4. 本地不能复现 / infra 问题 / 2 轮仍红 → **停**，输出失败 check 名 + 日志摘要 + PR URL + `$WT_ABS`，交人工

## 6. 等 codex review 评论

step 4 已通过独立 comment mention `@codex`，review bot 会被触发上来 review。轮询直到**Codex 至少有 1 条评论或 review** 才放行——没拿到 Codex 反馈就不 merge。

```bash
cd "$WT_ABS"

TIMEOUT_SEC=1800   # 30 min（codex 平均 5–15 min，留 buffer）
INTERVAL=60
START=$(date +%s)
SEEN_CODEX=0

while :; do
  ELAPSED=$(( $(date +%s) - START ))

  # 拉所有 review + issue 评论的 author，按小写匹配（账号名各种变体：codex / chatgpt-codex-connector）
  AUTHORS=$(gh pr view "$PR_NUM" --json comments,reviews \
    --jq '[.comments[].author.login, .reviews[].author.login] | .[]' 2>/dev/null \
    | tr '[:upper:]' '[:lower:]' | sort -u)

  echo "$AUTHORS" | grep -qE 'codex'  && SEEN_CODEX=1

  echo "[wait-review] elapsed=${ELAPSED}s codex=$SEEN_CODEX"

  [ "$SEEN_CODEX" = "1" ] && break
  [ "$ELAPSED" -ge "$TIMEOUT_SEC" ] && break
  sleep "$INTERVAL"
done

echo "[review-status] codex=$SEEN_CODEX elapsed=${ELAPSED}s"
```

放行条件分两种：

- **Codex 到** (`codex=1`) → 拉评论原文进 step 7
- **Codex 没到**（30 min 仍 0）→ **停，不 merge**。这种通常是 mention 没被 bot 捕获或 bot 离线，输出 PR URL + `$WT_ABS` 交人工

拉评论原文（Codex 到后拉）：

```bash
cd "$WT_ABS"
gh pr view "$PR_NUM" --json comments,reviews,reviewRequests \
  --jq '{comments: [.comments[] | {user: .author.login, body: .body, at: .createdAt}],
         reviews: [.reviews[] | {user: .author.login, state: .state, body: .body, at: .submittedAt}]}'

# 行内代码评论也要拉（review 的 inline thread）
gh api "repos/{owner}/{repo}/pulls/$PR_NUM/comments" \
  --jq '[.[] | {user: .user.login, path: .path, line: .line, body: .body}]'
```

## 7. 评审意见处理（自行判断）

读 step 6 拿到的所有评论 + 行内评论 + review state，**自行评估严重性**，决定采纳/修复/忽略，不要照单全改也不要凡异议必停。**本 step 终点固定是 step 8（自动 merge）或 step 7.3（交人工）——不存在"卡在这里等人确认"的第三态**。

### 7.0 强阻断信号（自动归入"严重"）

下列任一出现，无需评估直接走 7.2 严重分支必修：

- 任意 reviewer 的 review state = `CHANGES_REQUESTED`
- 评论正文含 `BLOCKING:` / `BLOCK:` / `MUST FIX` / `必须修复` / `阻断` 前缀或显眼标注
- 评论明确点名"plan 目标矛盾" / "破坏 worldview" / "破坏现有运行时" / "introduces regression"

### 7.1 无需处理 → 直接进 step 8

- 没有任何 review/评论（且 step 6 已记录 reviewer 缺席）
- 评论是纯肯定或纯描述："✅ LGTM" / `APPROVED` review / "整体没问题" / "符合 CLAUDE.md 约定" / 只复述改动
- 评论与本 PR 改动无关（闲聊、未来规划、其他 PR）

### 7.2 自行评估，按严重性分桶

- **严重（bug / 安全 / 破坏运行时 / 严重违反 CLAUDE.md / 与 plan 目标矛盾 / 与 worldview.md 冲突 / 命中 7.0 强阻断信号）**：必须修
  - 走 step 3 的"有限修复循环（≤2 轮）"：atomic fix commit + 跑对应子项目测试 + `cd "$WT_ABS" && git push`
  - 修完**回 step 5 重等 CI 全绿**，再回本 step；CI 过 + 已修严重项 → 直接进 step 8（不重新等 review，避免无限循环）
- **中等（明确质量问题但不影响功能：错误处理缺失、命名误导、文档与代码不符、明显的边界 case 未处理）**：自行决定
  - 决定修 → 同上有限修复，CI 过后进 step 8
  - 决定不修 → 在 PR 内 `gh pr comment "$PR_NUM" --body "..."` 回一句理由，最终输出里同步写"未采纳：<理由>"，进 step 8
- **轻微（nit / style / 主观偏好 / "可以考虑"/"建议"语气 / "更好的做法是"）**：默认不采纳
  - 在 PR 内统一回一条 `gh pr comment` 说明（"已评估 N 条 nit / 主观建议，未采纳；如需阻断请加 BLOCKING: 重开 review"），最终输出同步记录，进 step 8

判断原则：

- 不确定严重性时往严重靠（宁可多修一轮）
- 但单纯偏好/风格/"也许更好"类建议不采纳是合法选择，**写一行理由**就行
- 修复范围严格限于"评论指向的具体问题"——禁止顺手重构、扩大改动面
- 行内评论不再视为强制信号——按内容严重性走分桶

### 7.3 交人工

下列情况停下交人工，不要硬撑：

- 修改建议涉及超出当前 plan 范围的结构性改动（跨模块重构、新增 plan 才能完成）
- 评论指向 plan 本身设计问题（不是 patch 实现问题）——人来决定要不要回炉
- 走了 ≥2 轮修复仍跑不过测试
- 自己确实读不懂评论在说什么，且无法在 worktree 上下文里定位

### 7.4 最终输出（无论走哪条路径都要给）

1. 一句话判定："收到 X 条意见，处理结果：已修 N / 未采纳 N / 交人工 N"
2. 分桶列出原文 + 处理方式（含未采纳的一行理由）
3. 已修的 atomic fix commit hash
4. PR URL + `$WT_ABS`

## 8. Merge（自动）

step 7 过关（含 7.1 / 7.2 不修或修完且 CI 重绿）→ 直接 squash merge，不再人工确认：

```bash
cd "$WT_ABS"
gh pr merge "$PR_NUM" --squash --delete-branch
```

merge 失败兜底（远端被新 commit 抢跑 / branch protection 临时变化等）：

```bash
# 等 GitHub 状态稳定再确认一次
sleep 3
MERGED=$(gh pr view "$PR_NUM" --json state --jq '.state')
[ "$MERGED" = "MERGED" ] || { echo "❌ merge 未生效，state=$MERGED，PR=$PR_URL，WT=$WT_ABS"; exit 1; }
echo "[ok] merged: $PR_URL"
```

## 9. 收尾清理（此处才允许回主工作区）

```bash
cd "$REPO_ROOT"
git worktree remove "$WT_ABS"
git branch -D "$BRANCH" 2>/dev/null || true
```

最终输出：

```
✅ plan-$PLAN 已 merged: $PR_URL
```

---

## 失败时的统一行为

任何 step 停下时：

- 输出：哪一步挂的、错误原文、**`$WT_ABS` 路径**、PR URL（若已开）
- **不**自动重试（超出 step 3 / step 5 中明确允许的 ≤2 轮修复）
- **不**清理 worktree / 分支 / PR
- 处理 review 评审意见的边界见 step 7（自行判断采纳/修复/忽略；范围外、设计层、反复修不过才交人工）
- **不** `cd` 回主仓库根目录（除非已到 step 9）
- 把控制权完全交给人
