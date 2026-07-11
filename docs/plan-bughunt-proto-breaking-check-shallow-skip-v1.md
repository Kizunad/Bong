# plan-bughunt-proto-breaking-check-shallow-skip-v1

> 状态：活跃 BugHunt plan，可被流水线消费。分区：e2e-protocol。主题：PR 阶段 protobuf breaking gate 因 shallow checkout / base ref 缺失被误判为“base 无 proto”，导致破坏性协议变更可假绿。

## 一句话 bug

`.github/workflows/e2e.yml` 的 `Proto breaking change check (buf)` 在 PR 上直接引用 `origin/${{ github.event.pull_request.base.ref }}`，但 `actions/checkout@v4` 默认只拉取触发 workflow 的单个提交；当 runner 没有该 remote ref 时，`git ls-tree` 的 fatal 被 `| grep -q .` 吞成 `else` 分支，workflow 打印 `proto/ not found on base branch -- skipping breaking check (first PR)` 并继续绿。

## 第一性原理判断

`.proto` 是 server/client/agent 共享 wire contract。兼容性不能只看当前 PR 内部是否能一起编译，因为同一个 PR 可以同时更新两端让现有测试绿，却仍破坏已经发布或正在运行的旧 payload。`buf breaking` 的必要性就在于把 PR 版本和 base branch 的已知协议基线对比。

因此，PR gate 的最低不变式是：如果 base branch 应当存在 `proto/`，breaking check 必须拿到可验证的 base object；拿不到 base ref 是校验环境错误，不能等价为“base 没有 proto，可以跳过”。当前浅拉缺 base ref 时，唯一跨版本基线被移除，gate 退化成只做 `buf lint`。

e2e/protocol 的失败模式是“同 PR 自洽但跨版本破坏”：删除字段、修改字段类型、复用字段编号或删除 oneof 分支，可能仍通过当前 server/client/agent 同步构建和有限 smoke；合入后旧客户端、bot、Tiandao agent 或回放 payload 才表现为解析失败、HUD 状态丢失、技能/物品交互静默断链。

## 实际游玩体验影响

这不是单纯 CI 文案问题，而是会放过 `.proto` 的破坏性变更。若字段编号被复用、oneof 变体被删除、消息字段类型改变，PR 阶段本应由 `buf breaking` 拦截；当前 gate 可能直接跳过。合入后玩家会看到 server/client/agent 协议断链：HUD 状态不更新、技能/物品交互 payload 解析失败、天道事件或客户端反馈静默丢失，而 CI 给出“协议兼容”的假安全信号。

## 根因证据

- `.github/workflows/e2e.yml:37-38`：`actions/checkout@v4` 未配置 `fetch-depth: 0`，也没有显式 fetch base branch。
- `.github/workflows/e2e.yml:86-94`：breaking check 使用 `git ls-tree origin/${{ github.event.pull_request.base.ref }} proto/ | grep -q .` 判断 base 是否有 `proto/`。
- `.github/workflows/e2e.yml:90-93`：缺失 base ref 时进入 `else`，把“无法读取 base ref”误报为“base branch 没有 proto/，跳过 first PR”。
- `proto/buf.yaml:1-13`：仓库明确配置了 buf lint + breaking change 检测，breaking gate 不是可选装饰。
- 本地 shell 复现：

```text
fatal: Not a valid object name origin/__definitely_missing_base_ref__
ELSE
pipeline_status=0
```

上述命令等价于当前 workflow 的判断形态：`git ls-tree` fatal 后，`grep -q` 没有输入并返回非零，`if` 进入 `else`；由于 `else` 只是 `echo`，整个 step 仍可成功。

## 触发路径

1. 普通 PR 修改 `proto/bong/envelope.proto`，引入 buf 应判定为 breaking 的变更，例如删除已发布字段、改字段类型、复用字段编号。
2. GitHub Actions 用默认 `actions/checkout@v4` checkout PR 触发提交；runner 没有 `origin/main` 或对应 base ref。
3. `git ls-tree origin/${{ github.event.pull_request.base.ref }} proto/` 报 `fatal: Not a valid object name ...`。
4. pipeline 没有 `pipefail`，`grep -q .` 让 `if` 走 `else`。
5. workflow 打印 `proto/ not found on base branch -- skipping breaking check (first PR)`，没有执行 `buf breaking --against ...`。
6. 后续 schema / agent / server / e2e 阶段可能仍绿，破坏性 wire 变更失去 PR 阶段门禁。

## 去重说明

- 不重复 #1109：#1109 是 e2e Redis 命令锚点假绿，本题是 `buf breaking` 对 base ref 的读取与跳过逻辑。
- 不重复 #1054 / #1059 / #1075 / #1081 / #1093 / #1098：这些覆盖具体 agent/schema/server_data/Redis 事件断链，本题是 CI 协议兼容 gate 本身被跳过。
- 不重复 `docs/plans-skeleton/plan-bughunt-client-request-schema-drift.md`：该 plan 是 TS `ClientRequestV1` union 与 Rust/Java 漂移，本题是 protobuf `.proto` breaking check 没有真实运行。
- 不重复 `docs/plans-skeleton/plan-bughunt-bot-combat-server-data-type-false-positive-v1.md`：该 plan 是 bot 场景对 protobuf `server_data` 类型断言假阳性，本题是 PR workflow 的 buf baseline 缺失。

## 执行 TODO

- [ ] TODO 1（P0）：让 checkout 拉到 breaking check 需要的 base ref。可选方案：
  - `actions/checkout@v4` 配 `fetch-depth: 0`；
  - 或在 breaking step 前显式 `git fetch origin ${{ github.event.pull_request.base.ref }} --depth=1`，并用 `FETCH_HEAD` / 本地 base ref 作为 `--against`。
- [ ] TODO 2（P0）：在 breaking check step 开头加 `set -euo pipefail`，禁止 `git ls-tree` fatal 被 pipeline 吞掉。
- [ ] TODO 3（P0）：把“base ref 缺失”和“base branch 确实没有 proto/”拆成两个分支；前者必须 fail，不允许 skip。
- [ ] TODO 4（P1）：补一个 workflow 级脚本测试或 shellcheck 风格测试，模拟 missing base ref 时断言 step 失败，模拟 base 有 `proto/` 时断言执行 `buf breaking`。
- [ ] TODO 5（P1）：在 PR evidence 中打印实际 against target，便于 review 时确认 breaking gate 没有退化为 no-op。

## 验收测试计划

- [ ] 构造本地 missing ref：`git ls-tree origin/__missing__ proto/ | grep -q .`，修复后对应 helper/脚本必须非零退出，不得打印 skip。
- [ ] 在有 base ref 的环境运行 workflow breaking step，确认调用 `buf breaking --against ...`。
- [ ] 人工做一个临时 breaking proto diff，确认 PR 阶段被 `buf breaking` 拦截。
- [ ] 再做一个非 breaking proto diff，确认 `buf lint` + `buf breaking` 均通过。

## 对抗结论

第一轮反方质疑：本地 worktree 有 `origin/main`，也许 GitHub runner 同样有 base ref；`buf lint` 和后续 Rust/Java/TS 构建也许足以兜底。

第一轮复核：驳回。`actions/checkout@v4` 默认不是全分支 checkout，当前 workflow 没有 `fetch-depth: 0` 或显式 fetch base；`buf lint` 只检查语法/风格，不检查对已发布 proto 的 breaking change。`proto/buf.yaml` 明确配置了 breaking gate，说明仓库需要这道 PR 门禁。

第二轮反方质疑：即便 missing ref 会走 `else`，这是否只是 CI 测试债，不足以算实际 bug；push 到 main 后其他检查可能暴露问题。

第二轮复核：通过。breaking change 的正确拦截点就是 PR 阶段；合入后再发现已晚，且很多 wire 破坏不会在同一个 PR 的现有 e2e 中被覆盖。该缺口会让玩家可见协议断链以“CI 绿”的形式进入主干，属于高置信 e2e/protocol 假绿。
