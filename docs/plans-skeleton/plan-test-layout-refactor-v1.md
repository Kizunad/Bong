# plan-test-layout-refactor-v1 — 四栈测试布局盘点、所有权冻结与统一入口设计

> **一句话主题**：在不搬迁任何现有测试文件的前提下，冻结 server/client/agent/worldgen 四栈的测试目录、执行命令、CI job 与报告产物所有权，设计后续可实现的 `scripts/test-all.sh` 统一入口。
>
> **当前状态**：骨架（skeleton），T0 设计盘点已完成；未创建统一脚本，未修改现有测试路径、workflow、构建配置或测试代码。
>
> **盘点基线**：2026-08-23，专用 worktree `.agent-worktrees/test-refactor-init`（分支 `plan-test-layout-refactor-v1`，基于当前 `origin/main`）。数量是目录扫描快照，不是测试用例总数；新增测试后以本矩阵的路径/命令契约为准。

| 阶段 | 主题 | 状态 |
|------|------|------|
| T0 / P0 | 四栈盘点、CI/产物地图、所有权矩阵冻结、统一入口契约设计 | ✅ 2026-08-23 |
| P1 | 只新增 `scripts/test-all.sh` 编排层，保持所有测试源路径不变 | ⬜ |
| P2 | CI job 以兼容层接入统一入口，保留现有 job 的证据与 DAG 语义 | ⬜ |
| P3 | 报告格式/保留策略与失败诊断收口；决定是否拆出 JUnit/coverage 转换 | ⬜ |
| P4 | 经过一轮 CI 对拍后，才评估是否需要任何测试路径整理 | ⬜ |

## 为什么独立成 skeleton

- 这不是某个业务模块的测试补充，而是跨 `server/`、`client/`、`agent/`、`worldgen/` 和 `.github/workflows/` 的测试基础设施设计；与现有 feature plan 及 `plan-refactor-master-v1` 的代码所有权不同。
- 当前已有多个局部入口：`scripts/smoke-test.sh`、`scripts/smoke-test-e2e.sh`、`scripts/smoke-tiandao-fullstack.sh`、worldgen preview、resource-pack 与 script-contract workflow。直接改其中任一入口会把盘点、迁移和行为改变混在一个 PR，故先独立冻结基线。
- 本 skeleton 不占用任何现有测试目录，不给既有测试重新命名，也不回写 `docs/CLAUDE.md`、`docs/worldview.md` 或其他 plan。

## 立 plan 前预检记录（T0，2026-08-23）

- **`docs/worldview.md`**：证据范围为 `docs/worldview.md:1-1734`（`wc -l` = 1734；文件首个世界观章节从 `:1` 开始，玩法/区域/经济等锚点覆盖全文）。对该完整范围执行 `grep -nEi 'test|测试|CI|脚本|统一入口|所有权|artifact|报告'`；命中的“入口”等词均属于玩法/地理语境（例如 `docs/worldview.md:1409`），没有测试目录、测试命令、CI job、报告或 artifact ownership 的基础设施决策。因此“不修改 worldview”的结论落在本 plan 的 `§接入面`（worldview 锚点）、`§T0/P0`（只盘点既有契约）和 `§验收抓手`（明确不改 `docs/worldview.md`），本 plan 不修改该文件。
- **`docs/finished_plans/`**：共 359 份归档 plan；相关关键词命中的是业务 plan 内的测试段（如 `plan-dandao-path-v1`、`plan-shield-block-combat-event-feedback-v1`），没有覆盖四栈测试布局、统一入口或 artifact ownership 的既有 plan，因此不并入。
- **当前 active `docs/plan-*.md`**：逐项检查了 `plan-bot-e2e-coverage-v1`、`plan-ci-redis-pull-resilience-v1`、`plan-refactor-master-v1` 及其他 active plan；前者负责 bot 场景覆盖，后者负责 CI Redis 稳定性，`plan-refactor-master-v1` 的矩阵是代码 ownership，均不拥有四栈测试目录/报告编排，不重复其 scope。
- **`docs/plans-skeleton/` 与 `reminder.md`**：立项前有 166 个 skeleton；无同名 `plan-test-layout-refactor-*` 或四栈测试布局主题骨架，`docs/plans-skeleton/reminder.md` 也无匹配待办。本文件因此作为独立 skeleton 新建。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：四栈现有测试源目录、各栈 manifest/build config、根 `scripts/` 的 smoke/contract 脚本、`.github/workflows/*.yml` 的 job DAG 与 artifact 配置。
- **出料**：本 plan 的 T0 盘点表、冻结的测试 ownership/producer/consumer 矩阵，以及 P1 可直接实现的 `scripts/test-all.sh` CLI 和报告契约；不产生运行时代码、schema、payload 或游戏玩法事件。
- **共享类型 / event**：无新增类型或 event；统一入口只能调用既有命令，不得把测试 helper 复制到新目录。
- **跨仓库契约**：不改 server↔agent↔client IPC。现有 schema generated/dist 文件、S2C fixture、Redis smoke、bot protocol 和 worldgen preview 只作为被编排的既有契约。
- **worldview 锚点**：不涉及玩法命名、境界、经济或区域数据；不修改 `docs/worldview.md`。
- **qi_physics 锚点**：不涉及真元/灵气计算；统一入口不得添加任何 gameplay 物理常数或替代现有 ledger 测试。

## T0 / P0 — 四栈现状盘点与边界冻结 ✅ 2026-08-23

### 1. Server（Rust / Cargo）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | 内联单测分布在 `server/src/**`（当前约 35 个 test-like Rust source，如 `*/tests.rs`、`*_test.rs`、`tests/` 模块）；外部集成测试在 `server/tests/*.rs`（当前 5 个入口文件）；性能基准在 `server/benches/chunk_generation.rs`、`server/benches/nbt_stamp.rs`。库+bin 拆分让 bench 直接调用生产代码，见 `server/src/lib.rs:1-10`。 |
| 依赖/配置 | `[dev-dependencies]` 使用 `wiremock` 与 Criterion HTML reports，`server/Cargo.toml:71-84`；Valence/Bevy 等运行时依赖仍由 Cargo 管理。 |
| 本地命令 | `cd server && ../scripts/build-token.sh cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test`（默认完整单元+集成）；定向 `cargo test <filter>` / `cargo test --test <name>`；基准 `cargo bench --bench chunk_generation` 或 `nbt_stamp`。完整仓库 smoke 由 `scripts/smoke-test.sh:16-23` 调用。 |
| CI job | `.github/workflows/e2e.yml`：`preflight`、`server-test`（`cargo test`）、`build-release`（release binary）、`smoke`；`bot-e2e` 两 shard 和 `chat-window` 通过 artifact 消费 server/schema。`script-contracts.yml` 另测 cargo 配置、slot、janitor、provenance、owned-artifacts；`worldgen-preview.yml` 的 `snapshot` 还会 release-build server。 |
| 报告/产物 | Cargo 默认 stdout/stderr；本地编译/测试文件在 `server/target/**`，Criterion HTML 在 `server/target/criterion/**`（不承诺 CI 上传）；E2E 上传 `.sisyphus/evidence/**`（`evidence-server-test`、`evidence-smoke`、`evidence-bot-shard-*`、`evidence-chat-window`）；`build-release` 上传 `bong-server-release`（binary + `manifest.json`）；preview 失败时上传 `bong-preview-server-log-*`。 |
| 冻结 owner | **Server owner** 负责 `server/src/**`、`server/tests/**`、`server/benches/**` 的测试语义和命令；**CI/DevEx owner** 负责脚本 wrapper、日志收集和 artifact plumbing。统一入口不得把 server 测试移入 `scripts/`。 |

### 2. Client（Fabric / Gradle / JUnit + GameTest）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | `client/src/test/java/**` 当前 518 个 Java 测试源文件，资源 fixture 在 `client/src/test/resources/**`；独立 GameTest 源在 `client/src/gametest/java/**`（当前 1 个入口）。 |
| 依赖/配置 | JUnit Jupiter 5.10，Java 17，见 `client/build.gradle:84-92`；`test` 使用 JUnit Platform，且显式 `dependsOn(runGametest)`，见 `client/build.gradle:202-217`。GameTest 报告路径由 `fabric-api.gametest.report-file` 固定为 `client/build/gametest-results.xml`（`:126-134`）。 |
| 本地命令 | `cd client && ../scripts/build-token.sh gradle test`（包含 GameTest）；`gradle build`；定向 `gradle test --tests '<pattern>'`；GameTest 可单独 `gradle runGametest`；worldgen 预览使用 `gradle runClientPreview`（xvfb）。仓库 smoke 的 `test build` 入口见 `scripts/smoke-test.sh:45-59`。 |
| CI job | `.github/workflows/e2e.yml` 的 `client` job 执行 `gradle test`；`worldgen-preview.yml` 的 `snapshot` 执行 `runClientPreview` 并截图；`smoke`/bot/chat jobs 消费已构建 server，不替代 client 单测。 |
| 报告/产物 | Gradle 原生 `client/build/test-results/test/**/*.xml` 与 `client/build/reports/tests/test/**`；GameTest `client/build/gametest-results.xml`；预览截图 `client/run/screenshots/preview-*.png` 与 `preview-grid.png`；CI 另上传 `evidence-client` 和 `worldgen-snapshot-*`。统一入口只复制/索引这些产物，不改变 Gradle 的原生路径。 |
| 冻结 owner | **Client owner** 负责 Java/Gametest/fixture 的分类、JUnit/Gradle 选择器和视觉断言；**Worldgen owner** 只消费 preview harness 产物，不取得 client 单测源的所有权。 |

### 3. Agent（TypeScript / Vitest）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | `agent/packages/schema/tests/**` 当前 31 个测试文件；`agent/packages/tiandao/tests/**` 当前 67 个测试文件。两包均以 `*.test.ts` 为主，fixture/sample 另位于各包源码与 `agent/packages/schema/samples/**`。 |
| 依赖/配置 | workspace 根 `agent/package.json` 只有跨包 build；schema 的 `build/check/generate/generate:check/test` 在 `agent/packages/schema/package.json:19-24`；tiandao 的 `build/check/test/start:mock` 在 `agent/packages/tiandao/package.json:7-14`。`npm test` 对 tiandao 先跑 `tsc -p tsconfig.test.json --noEmit` 再 `vitest run`。 |
| 本地命令 | `cd agent && npm ci`；schema：`npm run build`、`npm run check`、`npm test`、`npm run generate`；tiandao：`npm run check`、`npm test`、可选 `npm run start:mock`；定向 Vitest 过滤器通过 `npm test -- <pattern>` 传递。 |
| CI job | `c2s-gate-matrix.yml` 的 `contract` 执行 schema `npm run check` + Python gate test；`e2e.yml` 的 `schema` 执行 build/check/test/generate 并产出 schema-dist，`agent` 下载 schema-dist 后执行 tiandao check/test；`smoke`、`chat-window` 和 bot jobs 执行 Redis/agent 联调。 |
| 报告/产物 | Vitest 默认 stdout（当前没有统一 coverage/JUnit reporter）；schema CI artifact `schema-dist` 包含 `agent/packages/schema/dist/**` 和 `generated/**`；失败/证据目录为 `evidence-schema`、`evidence-agent`、`evidence-smoke`、`evidence-chat-window` 下的 `.sisyphus/evidence/**`。`npm run generate` 的 generated JSON 是契约产物，不是测试报告。 |
| 冻结 owner | **Schema owner** 负责 TypeBox/source、samples、generated 对拍和 schema job；**Tiandao owner** 负责 `packages/tiandao/tests/**`、Redis/mock runtime；跨包 contract 由两者共同 review，不能在统一入口中偷偷重生成或覆盖 samples。 |

### 4. Worldgen（Python terrain pipeline + console）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | `worldgen/tests/**` 当前 44 个 Python 文件；`worldgen/scripts/terrain_gen/test_*.py` 当前 7 个；dev-only console 在 `worldgen/console/test/**` 当前 5 个 TypeScript 文件；preview validator 测试在 `scripts/preview/test_*.py`，资源/模型 builder 的 Python 测试留在根 `scripts/`，不算 terrain 目录迁移。Python 测试同时存在 `unittest`（主流）和少量 `pytest`（如 `test_console_server.py`，缺 dev deps 时 `importorskip`）。 |
| 依赖/配置 | 默认 `worldgen/setup.sh` 只装 numpy；`worldgen/requirements-dev.txt:1-14` 的 FastAPI/uvicorn/httpx 仅供 console dev/pytest。console package 的 `test`/`build` 在 `worldgen/console/package.json:7-21`。 |
| 本地命令 | `cd worldgen && python -m unittest discover -s tests -p 'test_*.py' -v`；terrain_gen 目录按 `python -m unittest ...` 或 discover；console：`cd worldgen/console && npm test` / `npm run build`；主流水线为 `bash worldgen/pipeline.sh`（实现见 `worldgen/pipeline.sh:1-55`），完整 dev reload 为 `bash scripts/dev-reload.sh`。pytest 仅在需要 FastAPI TestClient 的 console 测试时显式使用。 |
| CI job | `.github/workflows/worldgen-preview.yml` 的 `snapshot` 先跑 anvil/region/span 专项、选定 `worldgen/tests` 子集、`scripts/preview` validator，再执行 `pipeline.sh ... anvil ... spawn`、headless server/client、内容校验和拼图；没有一个现存 job 声称覆盖 `worldgen/tests/**` 全量。 |
| 报告/产物 | Python 默认 stdout，pytest 可留下本地 `.pytest_cache/**`；pipeline 产物包括 `generated/**/terrain-plan.json`、`terrain-fields-summary.json`、`rasters/manifest.json`、little-endian raster `.bin`、`*-preview.png`，anvil backend 另写 `world/region/r.*.mca`；CI 上传 `worldgen-snapshot-*`（client 5 角度截图 + raster PNG）和失败时 `bong-preview-server-log-*`。console `vite build` 产物为 `worldgen/console/dist/**`，当前不是 CI artifact。 |
| 冻结 owner | **Worldgen owner** 负责 terrain Python、raster/anvil/preview validator 与 console tests；**Client owner** 负责 preview harness 的 Java 代码和截图语义；两者共同维护 preview artifact 的消费契约。 |

### 5. 跨栈脚本与 CI job 地图

以下是 T0 已核对的 workflow/job 清单；job 名称和现有命令在 P2 前保持不变，统一入口只作为兼容编排层：

| Workflow | Jobs / 关键测试命令 | 主要 artifact / 报告 | 当前 owner |
|---|---|---|---|
| `.github/workflows/e2e.yml` | `preflight`（proto/build-token/signal/preview contract）；`client`（Gradle test）；`schema`（schema build/check/test/generate）；`agent`（tiandao check/test）；`server-test`（Cargo test）；`build-release`；`smoke`（Redis full smoke）；`bot-e2e` shard 1/2；`chat-window` | `schema-dist`、`bong-server-release`、`evidence-client/schema/agent/server-test/smoke/bot-shard-* /chat-window` | 各栈 owner + CI/DevEx；DAG 依赖由 CI/DevEx owner 维护 |
| `.github/workflows/worldgen-preview.yml` | `snapshot`：worldgen 专项 unittest + preview validator + pipeline + release server + headless client + R1/R2/R3 内容校验 | `worldgen-snapshot-*`、失败 `bong-preview-server-log-*` | Worldgen owner（pipeline/validator），Client owner（preview harness） |
| `.github/workflows/build-resourcepack.yml` | `build`：resourcepack/model Python unittest、构包、manifest/SHA1/server default 对拍；`publish-release` | `bong-resourcepack-<sha>`（zip、`.sha1`、manifest），随后发布 release asset | Client/asset owner + CI/DevEx |
| `.github/workflows/c2s-gate-matrix.yml` | `contract`：schema `npm run check` + `check_c2s_gate_matrix.py` + `scripts/tests/check_c2s_gate_matrix_test.py` | 当前无 upload artifact，失败日志为 job log | Schema owner + Server network owner |
| `.github/workflows/script-contracts.yml` | `script-contracts`：cargo profile、slot registry、wt janitor、provenance、owned-artifacts shell/Python tests | 当前无 upload artifact，失败日志为 job log | CI/DevEx owner |
| `.github/workflows/review-consumer-tests.yml` | `test`：固定 central/provider review contract checkout、Node `node --test`、central contract `npm test` | 当前无 upload artifact，失败日志为 job log；不属于四栈 gameplay 测试 | CI/DevEx/review-infra owner |

根脚本的额外入口（`scripts/smoke-test.sh`、`scripts/smoke-test-e2e.sh`、`scripts/smoke-tiandao-fullstack.sh`、`scripts/smoke-law-engine.sh`、`scripts/bot-e2e.sh`、`scripts/e2e-*.sh`、`scripts/tests/**`）继续保留原命令和场景语义；当前 `scripts/tests/` 有 13 个 contract 文件，`scripts/preview/` 有 1 个 validator test，根 `scripts/` 另有 12 个 Python 与 20 个 shell test-like 文件。T0 不把它们改造成互相调用的套娃，也不把脚本测试复制进四栈目录。

## T0 冻结：测试所有权 / 产物所有权矩阵 v1

矩阵一旦进入 active，新增测试必须落在下表已有 owner/path 组合中；改变 owner、canonical path、命令或 artifact 名称必须单独在 plan 的决议节记录并由相关 owner review。没有“统一入口 owner”可以接管测试语义。

| 资产类型 | Canonical source / producer | 测试执行 owner | 报告/产物 consumer | 冻结规则 |
|---|---|---|---|---|
| Rust unit/integration/bench | `server/src/**`、`server/tests/**`、`server/benches/**` | Server | Server owner；CI job 仅收集 | 不搬到 `scripts/`，不由 wrapper 重写断言 |
| Fabric JUnit/GameTest/fixtures | `client/src/test/**`、`client/src/gametest/**`、`client/src/test/resources/**` | Client | Gradle report、CI client、preview consumer | 不把 GameTest 混入 JUnit 源目录；不改 `build/**` 原生输出路径 |
| TypeBox schema | `agent/packages/schema/tests/**` + samples/generated | Schema | agent/server/client contract jobs | source/generated/sample 变更必须同 PR 对拍 |
| Tiandao runtime | `agent/packages/tiandao/tests/**` | Tiandao | Agent owner、Redis/E2E jobs | `npm test` 的 tsc 前置不能被统一入口省略 |
| Terrain Python | `worldgen/tests/**`、`worldgen/scripts/terrain_gen/test_*.py` | Worldgen | Worldgen owner、preview job | 默认 `unittest` 语义不因少量 pytest 测试而整体切换 |
| Worldgen console | `worldgen/console/test/**` | Worldgen console owner | 本地 console build/test consumer | 保持独立 `package-lock.json`/Vite package 边界 |
| Preview validator / resource/model tooling | `scripts/preview/**`、`scripts/test_build_resourcepack.py`、`scripts/models/test_*.py` | CI/DevEx + 对应资产 owner | snapshot/resourcepack jobs | 仍是根脚本资产，不冒充四栈单元测试 |
| Cross-stack smoke/E2E | `scripts/smoke-*.sh`、`scripts/e2e-*.sh`、`scripts/bot-e2e.sh` | CI/DevEx 编排；领域 owner 提供场景 | `.sisyphus/evidence/**`、job log、截图 | 统一入口只能调度，不复制场景逻辑或改变 Redis/时间/fixture 前置 |
| CI workflow/artifact plumbing | `.github/workflows/**`、artifact upload/download blocks | CI/DevEx | GitHub Actions artifacts/release | P2 只做兼容接入；artifact 名称和 retention 未决前不可改 |

## P1 设计目标 — `scripts/test-all.sh`（当前只设计，不创建）

### CLI 契约

建议入口位于仓库根 `scripts/test-all.sh`，默认从脚本所在目录解析仓库根，不依赖当前 cwd。P1 实现前不得把以下设计当成已落地命令：

```text
scripts/test-all.sh [--profile unit|contract|full|e2e|preview] \
                    [--suite server|client|schema|tiandao|worldgen|console|scripts] \
                    [--report-dir DIR] [--continue] [--list] [--help]
```

- `--profile unit`：四栈本地可重复测试（server `cargo test`、client `gradle test`、schema/tiandao `npm test`、worldgen unittest + console Vitest），其中 client 的 `gradle test` 必须保留现有 `dependsOn(runGametest)` 语义；不自动启动 Redis、真实 LLM 或世界生成大图。
- `--profile contract`：在 `unit` 之前/之后加入 `scripts/tests/**`、schema generated check、resourcepack/preview validator 等快速合同测试；不替代现有 workflow 的显式 job。
- `--profile full`：按依赖 DAG 运行 unit + contract + build；允许昂贵编译，但仍不启动长生命周期 Redis/E2E。
- `--profile e2e`、`--profile preview`：显式调用既有 `smoke-test-e2e.sh`、`bot-e2e.sh`、`e2e-chat-signal-window.sh`、`worldgen/pipeline.sh`/preview harness；默认不纳入 `unit`，避免本地入口意外消耗外部服务或 30 分钟 CI 预算。
- `--suite` 是选择器，可重复传入；未知 suite/profile、缺少 Java 17/Node/Python/Rust 或依赖未安装时必须以明确 `SKIP`/非零退出说明，不得静默成功。
- `--continue` 只影响后续 suite 是否继续；最终退出码仍为非零。无该 flag 时 fail-fast，但必须写出已跳过的 suite。
- `--list` 只打印矩阵中的 suite、命令、依赖和预期报告路径，不执行测试。

### 编排与报告契约

1. **依赖顺序**：先 preflight/工具探测，再 schema `check/build/generate`；其后 server、client、tiandao、worldgen unit 可按资源锁并行；contract/smoke/e2e/preview 只能在明确 profile 中运行。schema generated/dist 的生成仍由 schema producer 负责，不能让 agent job 隐式生成另一份；若未来拆成 `schema-dist` artifact，必须沿用同一 producer。
2. **资源锁**：server cargo 使用既有 `CARGO_TARGET_DIR`/`build-token.sh` 约定；client 使用 Java 17 与 Gradle wrapper；worldgen 输出目录和 preview run 目录必须 run-private。P1 不删除或清理共享缓存，不改变 `scripts/lib/smoke-owned-artifacts.sh` 的所有权判断。
3. **统一 envelope、原生报告不搬家**：每次运行生成一个可配置的 run-private report dir（建议默认 `.sisyphus/evidence/test-all/<run-id>/`），写入 `summary.json`、`summary.tsv`、每 suite 的 `command.txt`/`status`/`stdout.log`/`stderr.log`；Gradle XML/HTML、Criterion HTML、schema generated、raster/PNG/MCA 继续留在各自原生路径，仅在 summary 中索引。
4. **状态语义**：suite 状态固定为 `PASS`、`FAIL`、`SKIP`、`BLOCKED`；`BLOCKED` 仅用于缺失外部前置且 profile 明确要求它的情况。summary 必须包含 `profile`、git SHA、开始/结束时间、命令（脱敏）、工作目录、退出码、原生产物列表和 owner。
5. **退出码**：0 仅当所有要求 suite 为 PASS；1 为测试失败；2 为 usage/config/preflight 错误；3 为报告写入/产物完整性错误；`--continue` 不吞掉任何失败。管道命令必须读取 `${PIPESTATUS[0]}`，不能用 `tail` 制造假绿。
6. **CI 兼容**：P2 先让一个 job 以 `test-all.sh --profile unit --suite ...` 做 shadow/对拍，并继续执行原命令；只有 summary、退出码、原生报告和时限都对拍后，才考虑替换 job 内命令。artifact upload/download 名称、DAG needs 和 cleanup 语义在此之前不改。

## P2/P3/P4 预留交付物（当前不实施）

- **P2 CI 兼容接入**：为每个现有 DAG job 写 explicit suite mapping，先 shadow run，再逐 job 切换；保留 `schema-dist`、`bong-server-release`、`evidence-*`、`worldgen-snapshot-*`、resourcepack artifact 的生产/消费关系。
- **P3 报告统一**：对比 stdout、JUnit XML、Criterion HTML、Vitest stdout、unittest stdout、raster/PNG/MCA 与 `.sisyphus/evidence/**` 的诊断价值；只有确实需要跨 job 汇总时才增加 JUnit/coverage 转换器，转换器归 CI/DevEx owner，不修改源测试。
- **P4 路径整理评估**：只在 P1-P3 完成、CI 对拍和 owner 签字后评估；默认结论是“无需搬路径”。若确有迁移，必须另列迁移表、双跑窗口、回滚方案和 artifact 兼容期，不能借本 plan 的 T0/P1 顺手移动文件。

## 验收抓手（T0）

- `docs/plans-skeleton/plan-test-layout-refactor-v1.md` 独立存在，未产生 `scripts/test-all.sh`。
- 盘点覆盖四栈的 source directory、local command、CI job、native report/artifact；每行都有 owner 和 consumer。
- 矩阵明确区分测试语义 owner、编排 owner、报告 producer/consumer，且声明跨栈 smoke 不得复制场景。
- `git diff --name-only`（在干净基线核验时）只应出现本 skeleton；本轮不要求清理工作树中已有的用户文件。
- 本轮不修改任何 `server/**`、`client/**`、`agent/**`、`worldgen/**` 测试路径，不修改 `.github/workflows/**`，不添加依赖。

## §8 开放问题（升 active / P1 决策门前需收口）

1. **`unit` profile 是否包含根 `scripts/tests/**` 和 asset/model Python tests**：建议保留四栈 unit 与 `scripts` contract 分层，避免“全量”名字掩盖跨栈副作用；需 CI/DevEx owner 确认默认本地时限。
2. **统一入口是否并行**：server/client 编译并发、共享 Cargo target、Gradle daemon 和 worldgen 输出的资源上限需用一轮实测决定；在此之前只承诺 DAG，不承诺并行度。
3. **Vitest/pytest 是否引入统一 JUnit reporter**：现状 schema/tiandao/console 依赖 stdout，worldgen 仅少数 pytest；需先确认 GitHub artifact/检查器是否真正消费 JUnit，不能为格式统一而新增无消费者产物。
4. **CI 接入策略**：shadow run 的 job 选择、重复执行预算和失败归因窗口需要拍板；未经对拍不得删除现有显式命令。
5. **artifact retention 与命名是否冻结为现状**：`evidence-*`、`schema-dist`、`bong-server-release`、`worldgen-snapshot-*`、`bong-resourcepack-*` 已被 job/PR comment 消费，保留期和命名变更需 CI/DevEx 与各栈 owner 共同决议。
6. **worldgen console 的归属边界**：继续作为 worldgen 子包由独立 `package-lock.json` 管理，还是未来纳入 agent workspace；T0 结论是继续独立，改变需另行决议。
7. **矩阵的 owner 粒度**：当前按栈/基础设施角色冻结，不写个人姓名；若仓库建立 CODEOWNERS 或 team slug，P1 应把角色映射到可执行 reviewer，而不是改变测试路径。

### §8.1 决议要求（pre-P1）

每个开放问题必须补充“决议、实施命令/字段、边界条件、文件:行号 + plan 章节落点”。未完成 §8.1 前不得实现 `scripts/test-all.sh`，更不得把本 skeleton 升为 active。
