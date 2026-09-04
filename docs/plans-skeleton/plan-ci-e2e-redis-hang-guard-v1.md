# plan-ci-e2e-redis-hang-guard-v1

> **骨架（草案）**。一句话主题：给 CI smoke 的 Redis e2e 非 mock Tiandao 入口建立硬超时与 hermetic workspace 执行边界，让外部命令在远小于 job 上限处明确失败并保留诊断，而不是把 GitHub job 烧满后静默取消。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | `scripts/e2e-redis.sh` 非 mock Tiandao 调用的超时、失败 stage 与运行目录诊断 | ⬜ |
| P1 | 使用 workspace 内 `agent/node_modules/.bin/tsx`，缺失时 fail fast，禁止 `npx` 运行时拉包 | ⬜ |
| P2 | 扫描 `scripts/e2e-redis.sh` / `scripts/smoke-test-e2e.sh` 同类前台阻塞调用，补必要 contract pin，不改变测试语义 | ⬜ |
| P3 | 受影响脚本验证、故障注入证据、主线复验与归档 | ⬜ |

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
