# plan-ci-e2e-redis-hang-guard-v1

> **Active plan**。一句话主题：给 CI smoke 的 Redis e2e 非 mock Tiandao 入口建立硬超时与 hermetic workspace 执行边界，让外部命令在远小于 job 上限处明确失败并保留诊断，而不是把 GitHub job 烧满后静默取消。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | `scripts/e2e-redis.sh` 非 mock Tiandao 调用的超时、失败 stage 与运行目录诊断 | ✅ 2026-09-05 |
| P1 | 使用 workspace 内 `agent/node_modules/.bin/tsx`，缺失时 fail fast，禁止 `npx` 运行时拉包 | ✅ 2026-09-05 |
| P2 | 扫描 `scripts/e2e-redis.sh` / `scripts/smoke-test-e2e.sh` 同类前台阻塞调用，补必要 contract pin，不改变测试语义 | ✅ 2026-09-05 |
| P3 | 受影响脚本验证、故障注入证据、主线复验与归档 | ✅ 2026-09-05 |

## 接入面 checklist

- **进料**：`scripts/smoke-test-e2e.sh` 调用 `scripts/e2e-redis.sh`；`scripts/e2e-redis.sh` 启动既有 Redis、schema、server 与 `agent/packages/tiandao/src/task-13-one-tick.ts` 测试链。
- **出料**：保持既有 `wait_for_pattern` 锚点、`finalize_failure` 失败报告、Redis channel proof、server/agent 日志与 CI smoke job 的原有退出语义；超时只把原本无限等待变成可核验失败。
- **共享类型 / event**：不新增或修改 server、schema、wire、gameplay 类型；复用现有脚本的 `CURRENT_STAGE`、`TIANDAO_LOG`、`RUN_DIR` 与 `finalize_failure` 契约。
- **跨仓库契约**：纯脚本/CI 基建；不改 server、agent、client 的运行时协议或 Redis key，仅调用既有 Tiandao task-13 入口。
- **worldview 锚点**：不涉及玩法、命名、境界、经济或区域；不修改 `docs/worldview.md`。
- **qi_physics 锚点**：不涉及真元/灵气计算；不新增物理常数、ledger 路径或相关测试。

## Bug 证据与边界

已定位的主路径是 `scripts/e2e-redis.sh:1207-1229`：非 mock Tiandao 阶段以无 timeout 的前台 `npx tsx` 运行，后续三个 60 秒 `wait_for_pattern` 只有在该命令返回后才会执行。`npx` 在 workspace 缺少本地解析时还会尝试从 registry 安装 `tsx`，使 CI gate 依赖运行时网络。

本 plan 不改 gameplay、server、schema/wire 或测试场景；不放宽断言、不删除测试、不把 `finalize_failure` 变成 warning；不动 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 的语义。job 级 `timeout-minutes` 只有在不改变既有 DAG/cleanup 语义且确有必要时才评估，否则以脚本级硬超时和诊断为主。

## P0 — 有界非 mock Tiandao 阶段

- 为 task-13 的 workspace Tiandao 调用设置显式硬超时，超时退出必须经现有失败收口路径，写明 stage、耗时、`TIANDAO_LOG` 与 `RUN_DIR`。
- 保存并传播真实命令退出码；超时、启动失败和正常非零退出不能被 `set -e`、管道或后续 pattern 检查吞掉。
- 保留三个现有 `wait_for_pattern` 锚点及其 60 秒语义，只有进程在时间窗内返回后才继续检查。

## P1 — hermetic workspace 可执行文件

- 从 `agent/node_modules/.bin/tsx` 调用既有 workspace 依赖；入口不存在时立即输出可修复诊断并失败，不退化为 `npx` 下载。
- 不改 `agent` 包依赖版本、任务入口或 Tiandao 行为。

## P2 — 同类调用与 contract

- 盘点 `scripts/e2e-redis.sh` 与 `scripts/smoke-test-e2e.sh` 的前台外部命令，只有确属同一无限阻塞风险的调用才加最小保护。
- 用 bash syntax、脚本 contract 和静态检查锁住：目标调用带硬超时、使用 workspace `tsx`、错误诊断可见、既有 cleanup/断言不被绕过。
- 若不修改某个相邻调用，在 evidence 中记录其已存在的有界等待或生命周期收口依据。

## P3 — 验收与收口抓手

- 故障注入 workspace `tsx` 缺失或入口挂起，确认在硬超时内失败并输出 stage、耗时、日志路径与真实退出原因；正常入口仍执行原有三项 pattern proof。
- 运行 `bash -n`、相关 scripts contract tests、workflow/调用静态核验及卡片要求的本地验证；管道若存在必须取真实上游退出码。
- 最终 `git fetch origin && git merge origin/main` 后按受影响范围复验；完成无上下文 validator、PR/Kody/e2e 证据后再归档。

## 验收标准

1. `scripts/e2e-redis.sh` 的非 mock Tiandao 阶段不会无限等待，远小于 GitHub job 上限时给出可检索失败报告。
2. `tsx` 只来自 workspace 已安装入口；缺失时不会隐式访问 registry。
3. Redis、server、agent 的既有断言、cleanup、退出码与测试场景语义保持不变；正常 smoke 与故障注入结果均可从日志/evidence 核验。

## 后续

- 全部阶段完成并填充 Finish Evidence 后，按 BugFix 流程将本 skeleton promotion 为 active，最终归档为 `docs/finished_plans/plan-ci-e2e-redis-hang-guard-v1.md`。
- 不修改其他 plan、`docs/CLAUDE.md`、`docs/worldview.md`、Cargo、schema 或玩法。

## Finish Evidence

### 落地清单

- **P0**：`scripts/e2e-redis.sh` 的 `CURRENT_STAGE=tiandao` 阶段用 `timeout` 包住既有 task-13 one-tick 入口，默认 120 秒、TERM 后 5 秒 KILL；捕获并记录真实 wrapper 退出码、耗时、`TIANDAO_LOG` 和 `RUN_DIR`，超时/原生非零统一经过现有 `finalize_failure` 收口。原三个 60 秒 `wait_for_pattern` 锚点及 cleanup 顺序保留。
- **P1**：非 mock Tiandao 改用 `agent/node_modules/.bin/tsx`，缺失或不可执行时 fail fast 并给出 `npm ci` 修复提示；关键路径不再调用 `npx` 运行时访问 registry。
- **P2**：`scripts/e2e-redis.sh` 的 Redis probe、schema build、north-rift preview 与 `scripts/smoke-test-e2e.sh` 的前台阶段均使用有界执行；新增 `scripts/tests/e2e_redis_hang_guard_contract_test.py` 锁定 timeout、workspace tsx、native exit 与诊断语义，并由 `.github/workflows/e2e.yml` preflight 执行。未改 agent task、server/gameplay、schema/wire、client 或测试场景语义。
- **P3**：本分支从 `origin/main` 的 `d1b80331015211e439823859f1f6f39ce2f97e22` 合并到最新 `origin/main` `e7bfd6a4b5fdfafa96883dd0715b3ab6913315b5`，机械合并提交为 `6e328d110b0fa140dfcce6a7914fea7edbbb68f3`；归档前完成脚本验证与无上下文 validator，PR CI 继续承担干净 runner 上的完整 smoke/e2e 正常路径核验。

### 关键 commit

- `0add17c3b`（2026-09-05）：建立 `plan-ci-e2e-redis-hang-guard-v1` 骨架。
- `7ad796261`（2026-09-05）：将骨架 promotion 为 Active plan。
- `a99a86d49`（2026-09-05）：为 task-13 非 mock Tiandao 入口建立 workspace 执行与硬超时失败收口。
- `524325f84`（2026-09-05）：新增 hang-guard contract 并接入 e2e preflight。
- `7b7d0a50b`（2026-09-05）：补齐 native exit/timeout 动态对拍与相邻前台阶段边界。
- `6e328d110`（2026-09-05）：合并最新 `origin/main`，保留双方主线变更。

### 测试结果

- `bash -n scripts/e2e-redis.sh scripts/smoke-test-e2e.sh`：通过。
- `python3 scripts/tests/e2e_redis_hang_guard_contract_test.py`：5 tests passed，覆盖 workspace tsx、缺失入口、timeout、native exit 23 与日志诊断。
- `bash scripts/tests/test_all_contract_test.sh`：95 passed；`bash scripts/tests/build_token_test.sh`：23 passed；`python3 scripts/tests/ci_build_token_entrypoint_test.py`：11 passed；`python3 scripts/tests/signal_boundary_contract_test.py`：7 passed；`python3 scripts/tests/fallback_world_readiness_contract_test.py`：8 passed；`python3 scripts/tests/proto_breaking_base_ref_contract_test.py`：8 passed；preview lifecycle、cargo target scope、server provenance contract：通过。
- `git diff --check`：通过；workflow YAML 静态核验：通过。
- 真实 Redis/schema e2e 的 `RUN_LABEL=ci-e2e-redis-hang-guard-normal` 在 server release build 达到既有 600 秒 build guard，manifest 明确为 `status=FAILED stage=server`；随后 `BONG_E2E_BUILD_TIMEOUT_SECONDS=1800 RUN_LABEL=ci-e2e-redis-hang-guard-release-cached` 仍在首次 release 冷编译阶段达到 1800 秒，manifest 同样明确为 `stage=server`，未进入 Tiandao。两次均保留 `server.log` 与 run directory，未将编译环境失败伪报为 Tiandao 或正常 e2e 通过；动态 contract 已独立证明目标 timeout/native-exit 行为。
- 无上下文、read-only validator（模型 `gpt-5.6-luna`）对代码 HEAD `7b7d0a50b843fc9c96f5946f201e211b49304534` 返回 PASS；归档后的最终 HEAD 已重新执行同类 validator。

### 跨仓库核验

- **scripts/CI**：`scripts/e2e-redis.sh`、`scripts/smoke-test-e2e.sh` 与 `.github/workflows/e2e.yml` 仅增加有界执行和合同检查；原 Redis channel proof、server lifecycle、cleanup、pattern anchors、DAG 与 artifact 语义保留。
- **agent**：继续执行既有 `agent/packages/tiandao/src/task-13-one-tick.ts`，仅由 workspace 内已有 `tsx` 入口启动；未改 agent runtime、schema 或 Redis 协议。
- **server/client/schema**：无生产代码、Cargo 依赖、wire/schema、client 行为或 gameplay 改动；e2e 仍使用既有 server build 与 task-13 链路。

### 遗留 / 后续

- 本地 slot-4 首次 release 冷编译在 600 秒和临时 1800 秒 guard 内均未完成，故不宣称本地正常 e2e PASS；PR 的干净 CI runner 负责验证 server-test/smoke 正常链路，失败时保留原生 artifact。
- timeout 默认值、job timeout、三项 `wait_for_pattern` 与 cleanup 语义未放宽；缺失 workspace `tsx` 时仍 fail closed，不允许回退到 registry 下载。
- plan 已按 BugFix 三态流转归档；PR 开出后等待 CI/Kody/inline review，未经用户指示不自行合并。
