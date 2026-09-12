# plan-test-layout-refactor-v1 — 三栈测试外置布局、所有权冻结与统一入口设计

> **一句话主题**：以稳定业务契约为准保留必要测试，维护 server/client/agent 各自的 canonical 测试根；Rust 既有 inline tests 先分类，再按可回滚的小批次外置、合并、删除或保留独立私有测试模块；同时冻结执行命令、CI job 与报告产物所有权，落地 `scripts/test-all.sh` 统一入口。
>
> **当前状态**：Active plan；T0 设计盘点与“独立测试根 + 分阶段外置”决议已完成，P1 统一入口/owners/report contract 已交付；P2 已完成多批历史外置迁移，P3-01 server-test shadow 已完成。2026-09-05 起，未开始的迁移与已外置测试的收缩均采用本计划的契约筛选规则；P2、P3、P4 仍在进行。
>
> **盘点基线**：2026-08-23，专用 worktree `.agent-worktrees/test-refactor-init`（分支 `plan-test-layout-refactor-v1`，基于 `origin/main=eea1e73f2`）。数量是目录扫描快照，不是测试用例总数；新增测试后以本矩阵的路径/命令契约为准。外部 `BongWorldGen` 不属于本仓库或本 plan 的测试栈；Bong 仅保留 raster handoff 与 server/client preview 消费端。

| 阶段 | 主题 | 状态 |
|------|------|------|
| T0 / P0 | 三栈盘点、CI/产物地图、所有权矩阵冻结、统一入口契约设计 | ✅ 2026-08-23 |
| P1 | 测试放置规则、禁止新增 inline、`scripts/test-all.sh` 与 owners 映射层 | ✅ 2026-08-28 |
| P2 | Rust tests 外置与契约筛选策略重基线 | ⏳ |
| P3 | CI 兼容接入、报告收口与迁移对拍 | ⏳ |
| P4 | 剩余 Rust tests 按契约筛选后外置、合并、删除或保留私有模块，并完成全量回归 | ⬜ |

## 为什么最初独立成 skeleton

- 这不是某个业务模块的测试补充，而是跨 Bong 内 `server/`、`client/`、`agent/`、根 `scripts/` 和 `.github/workflows/` 的测试基础设施设计；与现有 feature plan 及 `plan-refactor-master-v1` 的代码所有权不同。
- 当前已有多个局部入口：`scripts/smoke-test.sh`、`scripts/smoke-test-e2e.sh`、`scripts/smoke-tiandao-fullstack.sh`、server/client preview handoff、resource-pack 与 script-contract workflow。直接改其中任一入口会把盘点、迁移和行为改变混在一个 PR，故先独立冻结基线。
- `BongWorldGen` 的生成器、console、terrain tests 与其 CI 已迁出到独立仓库；原 skeleton 只记录 Bong 侧的 raster handoff/preview 消费边界，不把外部测试重新算作第四栈。
- T0 阶段原 skeleton 不占用任何现有测试目录，不给既有测试重新命名，也不回写 `docs/CLAUDE.md`、`docs/worldview.md` 或其他 plan；P1 仅新增本 plan 指定的脚本入口、owners 映射与外置 contract pin。

## 立 plan 前预检记录（T0，2026-08-23）

- **`docs/worldview.md`**：证据范围为 `docs/worldview.md:1-1734`（`wc -l` = 1734；文件首个世界观章节从 `:1` 开始，玩法/区域/经济等锚点覆盖全文）。对该完整范围执行 `grep -nEi 'test|测试|CI|脚本|统一入口|所有权|artifact|报告'`；命中的“入口”等词均属于玩法/地理语境（例如 `docs/worldview.md:1409`），没有测试目录、测试命令、CI job、报告或 artifact ownership 的基础设施决策。因此“不修改 worldview”的结论落在本 plan 的 `§接入面`（worldview 锚点）、`§T0/P0`（只盘点既有契约）和 `§验收抓手`（明确不改 `docs/worldview.md`），本 plan 不修改该文件。
- **`origin/main` 栈边界**：`eea1e73f2`（`迁出旧 worldgen 到 BongWorldGen`）删除了顶层 `worldgen/` 与 `.github/workflows/worldgen-preview.yml`；`git ls-tree origin/main worldgen` 无结果，`CLAUDE.md:8,78-79` 与 `scripts/dev-reload.sh:438-461` 将生成器/console 定义为外部仓库。故 BongWorldGen 的测试目录、命令、CI job 与 artifact 不进入本 plan；Bong 保留的 `server/src/preview/**`、`client/preview-harness.json`、`scripts/preview/**` 归入 server/client/root-script 边界。
- **`docs/finished_plans/`**：共 359 份归档 plan；相关关键词命中的是业务 plan 内的测试段（如 `plan-dandao-path-v1`、`plan-shield-block-combat-event-feedback-v1`），没有覆盖当前三栈测试布局、统一入口或 artifact ownership 的既有 plan，因此不并入；历史 worldgen plans 不属于当前 Bong 测试栈。
- **当前 active `docs/plan-*.md`**：逐项检查了 `plan-bot-e2e-coverage-v1`、`plan-ci-redis-pull-resilience-v1`、`plan-refactor-master-v1` 及其他 active plan；前者负责 bot 场景覆盖，后者负责 CI Redis 稳定性，`plan-refactor-master-v1` 的矩阵是代码 ownership，均不拥有三栈测试目录/报告编排，不重复其 scope。
- **`docs/plans-skeleton/` 与 `reminder.md`**：立项前有 166 个 skeleton；无同名 `plan-test-layout-refactor-*` 或三栈测试布局主题骨架，`docs/plans-skeleton/reminder.md` 也无匹配待办。本文件当时作为独立 skeleton 新建，现已 promotion 为 `docs/plan-test-layout-refactor-v1.md`。

## Pre-P0 Decisions（T0，2026-08-23）

- **范围决策**：Bong 只冻结 server/client/agent 三栈与根脚本 contract/preview handoff；BongWorldGen 的生成器、测试和 CI 永不由 `scripts/test-all.sh` 隐式拉起。
- **preview handoff CLI 决策**：`--profile preview` 必须收到 `BONG_TERRAIN_RASTER_DIR`（优先，目录内含外部生成的 `focus-layout-preview.png`/`focus-surface-preview.png`）或 `BONG_TERRAIN_RASTER_PATH`（manifest 文件，取其父目录）；两者都缺失时 suite 为 `BLOCKED` 并返回非零。入口只读这些输入，不生成、覆盖或搬迁 raster；client 截图仍显式由 `--client-dir client/run/screenshots` 提供，`validate_snapshots.py` 只验证截图，`compose_grid.py` 负责拼图。
- **owner/reviewer 映射决策**：探索证据为 `find .github -maxdepth 2 -type f -iname '*codeowner*'` 无输出，且 `git ls-tree -r --name-only origin/main -- .github | grep -i codeowner`、`git ls-tree -r --name-only origin/main -- scripts/test-all.sh scripts/test-all-owners.tsv` 均无命中；因此不存在可引用的 `.github/CODEOWNERS`/目标文件行号，不再保留“未来若有 CODEOWNERS 再映射”的条件分支。当前可核验的 preview/CLI 证据锚点为 `client/build.gradle:231-248`、`scripts/preview/compose_grid.py:209-238`、`scripts/preview/validate_snapshots.py:212-220`；P1 目标文件固定为 `scripts/test-all.sh` 与 `scripts/test-all-owners.tsv`，落点见本文 `§P1 设计目标`/`§CLI 契约` 与 `§验收抓手`。P1 必须新增 owners TSV，以 `suite<TAB>owner_role<TAB>reviewer_path<TAB>evidence` 固定映射：`server→server/`、`client→client/`、`schema→agent/packages/schema/`、`tiandao→agent/packages/tiandao/`、`scripts→scripts/`；`scripts/test-all.sh --list` 必须逐行输出并校验该文件，缺行/路径不存在即 exit 2。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：三栈现有测试源目录、各栈 manifest/build config、根 `scripts/` 的 smoke/contract/preview handoff 脚本、`.github/workflows/*.yml` 的 job DAG 与 artifact 配置。
- **出料**：本 plan 的 T0 盘点表、冻结的测试 ownership/producer/consumer 矩阵，以及 P1 可直接实现的 `scripts/test-all.sh` CLI 和报告契约；不产生运行时代码、schema、payload 或游戏玩法事件。
- **共享类型 / event**：无新增类型或 event；统一入口只能调用既有命令，不得把测试 helper 复制到新目录。
- **跨仓库契约**：不改 server↔agent↔client IPC。现有 schema generated/dist 文件、S2C fixture、Redis smoke、bot protocol 和 server/client preview handoff 只作为被编排的既有契约；外部 BongWorldGen 生成器不由本 plan 调度。
- **worldview 锚点**：不涉及玩法命名、境界、经济或区域数据；不修改 `docs/worldview.md`。
- **qi_physics 锚点**：不涉及真元/灵气计算；统一入口不得添加任何 gameplay 物理常数或替代现有 ledger 测试。

## 测试放置决议（用户偏好，2026-08-26）

本 plan 采用“各栈独立测试根”，不建立跨 Rust/Java/TypeScript 的顶层 `test/`。目标是让生产文件只保留实现，让测试按其构建系统归属到可发现、可审查的测试目录。

| 栈 | canonical 测试位置 | 新增测试规则 | 现有 inline 迁移规则 |
|---|---|---|---|
| Rust | `server/tests/unit/**`、`server/tests/**`、`server/benches/**`；例外为经登记的 `server/src/**/tests.rs` | 不得把测试体继续写在生产实现文件；优先测试公开可观察行为。只有私有纯逻辑的必要契约无法经公开 API 验证、且外置会迫使生产 API 变形时，才可使用独立 `tests.rs` | 每个模块先做契约分类，再决定外置、合并、删除或保留独立 `tests.rs`；禁止为搬迁新增仅供测试调用的 `pub`/`#[doc(hidden)]` seam |
| Fabric | `client/src/test/java/**`、`client/src/test/resources/**`、`client/src/gametest/java/**` | 测试类、fixture、GameTest 分别放在既有 source set；生产 Java 不放测试方法 | 现有外置路径保持不变，仅在业务触及该模块时整理命名/目录 |
| Agent | `agent/packages/schema/tests/**`、`agent/packages/tiandao/tests/**` | Vitest 用例、samples/generated 对拍均放包级测试目录；生产 `src/**` 不新增测试体 | 现有外置路径保持不变，不把 schema 与 tiandao 合并成单一目录 |
| 根脚本 | `scripts/tests/**`、`scripts/preview/**`、显式 smoke/E2E 脚本 | contract/validator 与跨栈场景留在脚本根；不得复制到三栈目录 | 只整理归属和报告索引，不改变脚本的前置、Redis、时间或 artifact 语义 |

### 不变式与边界

1. **新测试先外置**：所有后续业务 PR 的任务卡必须附带本节；确有私有访问必要时，测试体放入独立 `tests.rs`，PR body 说明该私有契约、外置为何会扭曲生产 API，以及复审条件。
2. **迁移先分类，不以数量守恒为目标**：每个待迁移测试或同构测试组必须记录其受保护契约、风险和处置（保留、表驱动合并、替换或删除）。迁移前后的测试数量、名称、fixture 文案、随机种子和实现镜像断言不构成验收条件；已识别的协议、权限、守恒、状态转换和回归契约必须保持可验证。
3. **不机械破坏私有边界**：Rust 集成测试无法访问私有符号时，不通过扩大可见性、复制实现或新增仅供测试调用的 `pub`/`#[doc(hidden)]` seam 来“迁移”；优先验证公开可观察行为，必要时保留专用 `tests.rs` 并记录理由。P2 既有 seam 是历史迁移债务，P4 必须逐项审计并在不服务生产契约时收窄或删除。
4. **业务与测试重构分离**：大批量测试移动、收缩或 seam 清理单独成 PR；业务修复 PR 只新增能证明该改动风险的最小契约测试，避免 review 混入无关 rename 或测试数量竞赛。
5. **栈级门禁不变**：Rust 运行 `fmt --check`、`clippy --all-targets -- -D warnings`、`cargo test`；Client 运行 `gradle test build`（保留 `runGametest` 依赖）；Schema/Tiandao 运行各自 `npm test`，不得用统一入口省略原生前置。

## T0 / P0 — 三栈现状盘点与边界冻结 ✅ 2026-08-23

### 1. Server（Rust / Cargo）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | 内联单测分布在 `server/src/**`（当前约 35 个 test-like Rust source，如 `*/tests.rs`、`*_test.rs`、`tests/` 模块）；外部集成测试在 `server/tests/*.rs`（当前 5 个入口文件）；性能基准在 `server/benches/chunk_generation.rs`、`server/benches/nbt_stamp.rs`。库+bin 拆分让 bench 直接调用生产代码，见 `server/src/lib.rs:1-10`。 |
| 依赖/配置 | `[dev-dependencies]` 使用 `wiremock` 与 Criterion HTML reports，`server/Cargo.toml:71-84`；Valence/Bevy 等运行时依赖仍由 Cargo 管理。 |
| 本地命令 | `cd server && ../scripts/build-token.sh cargo fmt --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test`（默认完整单元+集成）；定向 `cargo test <filter>` / `cargo test --test <name>`；基准 `cargo bench --bench chunk_generation` 或 `nbt_stamp`。完整仓库 smoke 由 `scripts/smoke-test.sh:16-23` 调用。 |
| CI job | `.github/workflows/e2e.yml`：`preflight`、`server-test`（`cargo test`）、`build-release`（release binary）、`smoke`；`bot-e2e` 两 shard 和 `chat-window` 通过 artifact 消费 server/schema。`script-contracts.yml` 另测 cargo 配置、slot、janitor、provenance、owned-artifacts；根 `scripts/preview/**` 只作为 Bong 内 preview handoff 工具，不再有 worldgen snapshot job。 |
| 报告/产物 | Cargo 默认 stdout/stderr；本地编译/测试文件在 `server/target/**`，Criterion HTML 在 `server/target/criterion/**`（不承诺 CI 上传）；E2E 上传 `.sisyphus/evidence/**`（`evidence-server-test`、`evidence-smoke`、`evidence-bot-shard-*`、`evidence-chat-window`）；`build-release` 上传 `bong-server-release`（binary + `manifest.json`）。根 preview wrapper 的日志/截图仅在显式本地或调用方目录产生，当前没有独立 CI artifact。 |
| 冻结 owner | **Server owner** 负责 `server/src/**`、`server/tests/**`、`server/benches/**` 的测试语义和命令；**CI/DevEx owner** 负责脚本 wrapper、日志收集和 artifact plumbing。统一入口不得把 server 测试移入 `scripts/`。 |

### 2. Client（Fabric / Gradle / JUnit + GameTest）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | `client/src/test/java/**` 当前 521 个 Java 测试源文件，资源 fixture 在 `client/src/test/resources/**`；独立 GameTest 源在 `client/src/gametest/java/**`（当前 1 个入口）。 |
| 依赖/配置 | JUnit Jupiter 5.10，Java 17，见 `client/build.gradle:84-92`；`test` 使用 JUnit Platform，且显式 `dependsOn(runGametest)`，见 `client/build.gradle:202-217`。GameTest 报告路径由 `fabric-api.gametest.report-file` 固定为 `client/build/gametest-results.xml`（`:126-134`）。 |
| 本地命令 | `cd client && ../scripts/build-token.sh gradle test`（包含 GameTest）；`gradle build`；定向 `gradle test --tests '<pattern>'`；GameTest 可单独 `gradle runGametest`。`runClientPreview` 仍是显式的 client/server preview harness，不属于 unit 默认入口，也不负责生成 raster；preview profile 另需只读的 `BONG_TERRAIN_RASTER_DIR`/`BONG_TERRAIN_RASTER_PATH`。仓库 smoke 的 `test build` 入口见 `scripts/smoke-test.sh:45-59`。 |
| CI job | `.github/workflows/e2e.yml` 的 `client` job 执行 `gradle test`；`smoke`/bot/chat jobs 消费已构建 server，不替代 client 单测。没有 worldgen snapshot job。 |
| 报告/产物 | Gradle 原生 `client/build/test-results/test/**/*.xml` 与 `client/build/reports/tests/test/**`；GameTest `client/build/gametest-results.xml`；显式 preview harness 的截图 `client/run/screenshots/preview-*.png` 仅由调用方消费；CI 上传 `evidence-client`。统一入口只复制/索引这些产物，不改变 Gradle 的原生路径。 |
| 冻结 owner | **Client owner** 负责 Java/GameTest/fixture 的分类、JUnit/Gradle 选择器和 client preview harness 断言；外部 BongWorldGen 不取得 client 单测源的所有权。 |

### 3. Agent（TypeScript / Vitest）

| 项目 | 当前事实（基线） |
|---|---|
| 测试目录 | `agent/packages/schema/tests/**` 当前 31 个测试文件；`agent/packages/tiandao/tests/**` 当前 67 个测试文件。两包均以 `*.test.ts` 为主，fixture/sample 另位于各包源码与 `agent/packages/schema/samples/**`。 |
| 依赖/配置 | workspace 根 `agent/package.json` 只有跨包 build；schema 的 `build/check/generate/generate:check/test` 在 `agent/packages/schema/package.json:19-24`；tiandao 的 `build/check/test/start:mock` 在 `agent/packages/tiandao/package.json:7-14`。`npm test` 对 tiandao 先跑 `tsc -p tsconfig.test.json --noEmit` 再 `vitest run`。 |
| 本地命令 | `cd agent && npm ci`；schema：`npm run build`、`npm run check`、`npm test`、`npm run generate`；tiandao：`npm run check`、`npm test`、可选 `npm run start:mock`；定向 Vitest 过滤器通过 `npm test -- <pattern>` 传递。 |
| CI job | `c2s-gate-matrix.yml` 的 `contract` 执行 schema `npm run check` + Python gate test；`e2e.yml` 的 `schema` 执行 build/check/test/generate 并产出 schema-dist，`agent` 下载 schema-dist 后执行 tiandao check/test；`smoke`、`chat-window` 和 bot jobs 执行 Redis/agent 联调。 |
| 报告/产物 | Vitest 默认 stdout（当前没有统一 coverage/JUnit reporter）；schema CI artifact `schema-dist` 包含 `agent/packages/schema/dist/**` 和 `generated/**`；失败/证据目录为 `evidence-schema`、`evidence-agent`、`evidence-smoke`、`evidence-chat-window` 下的 `.sisyphus/evidence/**`。`npm run generate` 的 generated JSON 是契约产物，不是测试报告。 |
| 冻结 owner | **Schema owner** 负责 TypeBox/source、samples、generated 对拍和 schema job；**Tiandao owner** 负责 `packages/tiandao/tests/**`、Redis/mock runtime；跨包 contract 由两者共同 review，不能在统一入口中偷偷重生成或覆盖 samples。 |

### 4. 跨栈脚本与 CI job 地图

以下是 T0 已核对的 workflow/job 清单；job 名称和现有命令在 P2 前保持不变，统一入口只作为兼容编排层：

| Workflow | Jobs / 关键测试命令 | 主要 artifact / 报告 | 当前 owner |
|---|---|---|---|
| `.github/workflows/e2e.yml` | `preflight`（proto/build-token/signal/preview contract）；`client`（Gradle test）；`schema`（schema build/check/test/generate）；`agent`（tiandao check/test）；`server-test`（Cargo test）；`build-release`；`smoke`（Redis full smoke）；`bot-e2e` shard 1/2；`chat-window` | `schema-dist`、`bong-server-release`、`evidence-client`、`evidence-schema`、`evidence-agent`、`evidence-server-test`、`evidence-smoke`、`evidence-bot-shard-*`、`evidence-chat-window` | 各栈 owner + CI/DevEx；DAG 依赖由 CI/DevEx owner 维护 |
| `.github/workflows/build-resourcepack.yml` | `build`：resourcepack/model Python unittest、构包、manifest/SHA1/server default 对拍；`publish-release` | `bong-resourcepack-<sha>`（zip、`.sha1`、manifest），随后发布 release asset | Client/asset owner + CI/DevEx |
| `.github/workflows/c2s-gate-matrix.yml` | `contract`：schema `npm run check` + `check_c2s_gate_matrix.py` + `scripts/tests/check_c2s_gate_matrix_test.py` | 当前无 upload artifact，失败日志为 job log | Schema owner + Server network owner |
| `.github/workflows/script-contracts.yml` | `script-contracts`：cargo profile、slot registry、wt janitor、provenance、owned-artifacts shell/Python tests | 当前无 upload artifact，失败日志为 job log | CI/DevEx owner |
| `.github/workflows/review-consumer-tests.yml` | `test`：固定 central/provider review contract checkout、Node `node --test`、central contract `npm test` | 当前无 upload artifact，失败日志为 job log；不属于三栈 gameplay 测试 | CI/DevEx/review-infra owner |

根脚本的额外入口（`scripts/smoke-test.sh`、`scripts/smoke-test-e2e.sh`、`scripts/smoke-tiandao-fullstack.sh`、`scripts/smoke-law-engine.sh`、`scripts/bot-e2e.sh`、`scripts/e2e-*.sh`、`scripts/tests/**`）继续保留原命令和场景语义；当前 `scripts/tests/` 有 13 个 contract 文件，`scripts/preview/` 有 1 个 server/client preview validator test，根 `scripts/` 另有 12 个 Python 与 20 个 shell test-like 文件。`scripts/preview/**` 只验证 Bong 的 headless preview / 外部 raster handoff，不包含 terrain generator。T0 不把它们改造成互相调用的套娃，也不把脚本测试复制进三栈目录。

## T0 冻结：测试所有权 / 产物所有权矩阵 v1

矩阵一旦进入 active，新增测试必须落在下表已有 owner/path 组合中；改变 owner、canonical path、命令或 artifact 名称必须单独在 plan 的决议节记录并由相关 owner review。没有“统一入口 owner”可以接管测试语义。

| 资产类型 | Canonical source / producer | 测试执行 owner | 报告/产物 consumer | 冻结规则 |
|---|---|---|---|---|
| Rust unit/integration/bench | 迁移前 `server/src/**` + `server/tests/**`；目标 `server/tests/unit/**`、`server/tests/**`、`server/benches/**`，必要时为独立 `server/src/**/tests.rs` | Server | Server owner；CI job 仅收集 | 新测试不得进入生产实现文件；迁移 PR 先登记受保护契约，不新增测试专用 public seam，允许移除实现镜像断言 |
| Fabric JUnit/GameTest/fixtures | `client/src/test/**`、`client/src/gametest/**`、`client/src/test/resources/**` | Client | Gradle report、CI client、preview consumer | 不把 GameTest 混入 JUnit 源目录；不改 `build/**` 原生输出路径 |
| TypeBox schema | `agent/packages/schema/tests/**` + samples/generated | Schema | agent/server/client contract jobs | source/generated/sample 变更必须同 PR 对拍 |
| Tiandao runtime | `agent/packages/tiandao/tests/**` | Tiandao | Agent owner、Redis/E2E jobs | `npm test` 的 tsc 前置不能被统一入口省略 |
| Preview validator / resource/model tooling | `scripts/preview/**`、`scripts/test_build_resourcepack.py`、`scripts/models/test_*.py` | CI/DevEx + Server/Client/asset owner | 显式 preview/resourcepack callers | 仍是根脚本资产；preview CLI 只读 `BONG_TERRAIN_RASTER_DIR`/`BONG_TERRAIN_RASTER_PATH` 外部 handoff，不冒充三栈单元测试或外部生成器测试 |
| Cross-stack smoke/E2E | `scripts/smoke-*.sh`、`scripts/e2e-*.sh`、`scripts/bot-e2e.sh` | CI/DevEx 编排；领域 owner 提供场景 | `.sisyphus/evidence/**`、job log、截图 | 统一入口只能调度，不复制场景逻辑或改变 Redis/时间/fixture 前置 |
| CI workflow/artifact plumbing | `.github/workflows/**`、artifact upload/download blocks | CI/DevEx | GitHub Actions artifacts/release | P2 只做兼容接入；artifact 名称和 retention 未决前不可改 |

## P1 — 测试放置规则、统一入口与 owner 映射（✅ 2026-08-28）

P1 是独立基础设施 PR：创建 `scripts/test-all.sh`、`scripts/test-all-owners.tsv` 及其 contract tests，落地本节的新增测试放置规则；不在同一 PR 批量移动 `server/src/**` 内已有测试。

### P1 交付物

- `scripts/test-all.sh`：只编排既有命令，不能复制测试 helper、重写断言或隐式启动 Redis/LLM/BongWorldGen。
- `scripts/test-all-owners.tsv`：固定 suite、owner、reviewer path、证据路径；缺行或路径不存在时入口返回 exit 2。
- `scripts/tests/test_all_contract_test.sh`（或等价的 `scripts/tests/**` contract）：覆盖 `--help`、`--list`、未知参数、缺失工具、`--continue` 失败传播、`${PIPESTATUS[0]}` 退出码和 run-private 报告目录。
- `docs/` 只记录迁移表和决议，不把业务模块测试搬迁混进 P1。

### P1 实施证据（2026-08-28）

- `scripts/test-all.sh` 已落地从脚本自身解析仓库根；支持 `unit`/`contract`/`full`/`e2e`/`preview`、可重复 `--suite`、`--report-dir`、`--continue`、`--list`、`--help`，按串行 suite DAG 调用既有命令。unit 不包含 `scripts`，e2e 仅调用既有 smoke/bot/chat 三个入口，preview 只消费调用方提供的外部 raster 与 client 截图目录，不启动 BongWorldGen；preview server 日志经 `BONG_PREVIEW_LOG_FILE` 落入 run-private report。
- `scripts/test-all-owners.tsv` 固定覆盖 `server`、`client`、`schema`、`tiandao`、`scripts` 五行；`--list` 校验 header、恰好五个 suite、reviewer/evidence 路径存在，并输出七列 owners/command/dependency/native-report 矩阵。
- run-private 报告固定写入 `summary.json`、`summary.tsv` 及每个 suite 的 `command.txt`、`status`、`stdout.log`、`stderr.log`；状态固定为 `PASS`/`FAIL`/`SKIP`/`BLOCKED`，summary 索引原生报告路径并保留真实 `${PIPESTATUS[0]}` 退出码。
- `scripts/tests/test_all_contract_test.sh` 使用临时 fixture/stub 覆盖 help/list、未知参数/profile/suite、缺工具显式 SKIP、`--continue` 后续执行与最终非零、真实 native exit code `23`、unit client 的 `gradle test`、非 executable-bit 的既有 e2e bash 入口、preview 缺外部 raster 的 BLOCKED 语义、缺 xvfb-run 的显式 SKIP、preview 启动前及中断/退出 cleanup 注册、启动阶段不冒充 PID ownership、wrapper child PID/信号转发、stop 失败时保留 cleanup 状态、EXIT 重入与解除顺序、run-private server 日志/报告、owner/path 校验：`90 passed, 0 failed`；该结果在每个最终 HEAD 上重跑并由独立 validator 对拍。`scripts/tests/preview_lifecycle_contract_test.sh` 覆盖真实 wrapper 的 handoff marker 发布失败、identity-safe rollback 未确认、authority 保留与真实 stop 清理：`PASS`。
- 受影响门禁：`bash -n scripts/test-all.sh scripts/tests/test_all_contract_test.sh scripts/tests/preview_lifecycle_contract_test.sh scripts/preview/run-server-headless.sh scripts/preview/stop-server-headless.sh`、`git diff --check`、上述两个合同测试均通过；`shellcheck` 当前环境不可用，已记录并以 Bash 原生语法检查替代。真实 `contract` profile 对拍明确报告环境前置：`agent/node_modules` 未安装、resourcepack 既有构建缺少 `zip`，未静默成功；此前可运行的 modelScript validator 已通过 604 tests，resourcepack validator 因上述 `zip` 缺失非代码失败。
- review 修订 commit：`38daf9dbf`（2026-08-28，修复 preview authority handoff 失败清理）、`6c5bf9e25`/`f9b8cfd39`（2026-08-28，补齐真实 handoff 发布失败/rollback 未确认合同并隔离专用失败码）、`172a61141`/`98de9ed16`/`3f4fbb575`（2026-08-28，固定 identity handoff 对拍与 `renameat2(RENAME_NOREPLACE)` 独占发布），以及本次 evidence 修订提交；最终 fresh read-only validator 对当前 active plan 所在 HEAD 做严格 SHA 对拍并 PASS，精确 SHA 记录在 PR body，validator 模型为 `gpt-5.6-luna`。
- PR #2111 最新 CI/e2e 结果已如实记录：preflight、schema、agent、build-release、Script Contract、bot-e2e(2)、chat-window PASS；client 的 `R7InventoryContractTest.p1ProductionSourceTreeMatchesFrozenBaseline`（4973 中 1 失败）、server 的 `network::resourcepack::tests::committed_manifest_matches_default_constants`（12779 中 1 失败）及 bot-e2e(1) 的 `cultivation_qi_color_inspect`（81 PASS、1 FAIL、4 SKIP）均位于本 PR 未触及的固定测试/场景，未以越界修改掩盖。
- 本 P1 未新增生产 inline test、未移动既有测试、未改依赖版本、workflow、玩法/schema/worldview 或其他 plan；P2/P3/P4 仍保持 `⬜`。

### CLI 契约

入口位于仓库根 `scripts/test-all.sh`，从脚本所在目录解析仓库根，不依赖当前 cwd；同目录的 `scripts/test-all-owners.tsv` 是 owner/reviewer 映射真源。以下是已落地的 P1 CLI 契约：

```text
scripts/test-all.sh [--profile unit|contract|full|e2e|preview] \
                    [--suite server|client|schema|tiandao|scripts] \
                    [--report-dir DIR] [--continue] [--list] [--help]
```

- `--profile unit`：Bong 三栈本地可重复测试（server `cargo test`、client `gradle test`、schema/tiandao `npm test`），其中 client 的 `gradle test` 必须保留现有 `dependsOn(runGametest)` 语义；不自动启动 Redis、真实 LLM、外部 BongWorldGen 或 raster 生成。
- `--profile contract`：在 `unit` 之前/之后加入 `scripts/tests/**`、schema generated check、resourcepack/preview validator 等快速合同测试；不替代现有 workflow 的显式 job。
- `--profile full`：按依赖 DAG 运行 unit + contract + build；允许昂贵编译，但仍不启动长生命周期 Redis/E2E。
- `--profile e2e`：显式调用既有 `smoke-test-e2e.sh`、`bot-e2e.sh`、`e2e-chat-signal-window.sh`；默认不纳入 `unit`，避免本地入口意外消耗外部服务或 30 分钟 CI 预算。
- `--profile preview`：先校验 `BONG_TERRAIN_RASTER_DIR`/`BONG_TERRAIN_RASTER_PATH` 与 `client/run/screenshots`，再调用 Bong 内 server/client preview handoff wrapper；外部 raster 缺失时返回 `BLOCKED`，不得启动 BongWorldGen、写入外部目录或把 `generated/snapshot` 当作隐式生成结果。
- `--suite` 是选择器，可重复传入；未知 suite/profile、缺少 Java 17/Node/Python/Rust 或依赖未安装时必须以明确 `SKIP`/非零退出说明，不得静默成功。
- `--continue` 只影响后续 suite 是否继续；最终退出码仍为非零。无该 flag 时 fail-fast，但必须写出已跳过的 suite。
- `--list` 只打印矩阵中的 suite、命令、依赖和预期报告路径，不执行测试。

### 编排与报告契约

1. **依赖顺序**：先 preflight/工具探测，再 schema `check/build/generate`；其后 server、client、tiandao unit 可按资源锁并行；contract/smoke/e2e/preview 只能在明确 profile 中运行。schema generated/dist 的生成仍由 schema producer 负责，不能让 agent job 隐式生成另一份；若未来拆成 `schema-dist` artifact，必须沿用同一 producer。外部 raster 只作为 preview 的已生成输入，不在入口内生成。
2. **资源锁**：server cargo 使用既有 `CARGO_TARGET_DIR`/`build-token.sh` 约定；client 使用 Java 17 与 Gradle wrapper；Bong 内 preview 日志/截图目录必须 run-private，外部 raster 目录只读。P1 不删除或清理共享缓存，不改变 `scripts/lib/smoke-owned-artifacts.sh` 的所有权判断。
3. **统一 envelope、原生报告不搬家**：每次运行生成一个可配置的 run-private report dir（建议默认 `.sisyphus/evidence/test-all/<run-id>/`），写入 `summary.json`、`summary.tsv`、每 suite 的 `command.txt`/`status`/`stdout.log`/`stderr.log`；Gradle XML/HTML、Criterion HTML、schema generated 与 Bong 内 preview PNG/日志继续留在各自原生路径，仅在 summary 中索引；外部 raster 不由 Bong 入口生产或搬迁。
4. **状态语义**：suite 状态固定为 `PASS`、`FAIL`、`SKIP`、`BLOCKED`；`BLOCKED` 仅用于缺失外部前置且 profile 明确要求它的情况。summary 必须包含 `profile`、git SHA、开始/结束时间、命令（脱敏）、工作目录、退出码、原生产物列表和 owner。
5. **退出码**：0 仅当所有要求 suite 为 PASS；1 为测试失败；2 为 usage/config/preflight 错误；3 为报告写入/产物完整性错误；`--continue` 不吞掉任何失败。管道命令必须读取 `${PIPESTATUS[0]}`，不能用 `tail` 制造假绿。
6. **CI 兼容**：P3 先让一个 job 以 `test-all.sh --profile unit --suite ...` 做 shadow/对拍，并继续执行原命令；只有 summary、退出码、原生报告和时限都对拍后，才考虑替换 job 内命令。artifact upload/download 名称、DAG needs 和 cleanup 语义在此之前不改。

## P2 — Rust tests 外置与契约筛选策略（⏳）

历史 P2 外置记录保留其当时的一对一迁移证据，不倒改为新的结论。此后每个模块先完成契约分类：只保留能锁住稳定业务契约的测试；同构测试可表驱动合并；实现镜像和无外部风险的 fixture 断言可以删除或改为行为测试。首批复审优先选择 `server/tests/unit/shader/state_test.rs`、`server/tests/unit/world/dimension_test.rs`、`server/tests/unit/schema/client_payload_test.rs` 与 `server/tests/unit/world/tsy_container_search_test.rs`。

- **目标路径**：默认使用 `server/tests/unit/<module>_test.rs`，由 `server/src/lib.rs` 暴露的公开 API 驱动；只有外置会扭曲生产 API 的私有纯逻辑，才可保留独立 `server/src/**/tests.rs`。
- **私有访问**：不得为迁移新增仅供测试调用的 public seam。已有 `#[doc(hidden)] pub` seam 必须在复审中标注其生产消费者；没有生产消费者且无法改为公开行为测试时，测试应移入独立 `tests.rs` 或被删除。
- **验收**：迁移前后定向 `cargo test <filter>` 只用于确认构建发现和回归；验收依据是受保护契约仍被覆盖、删除项有分类理由，以及 server 完整 fmt/clippy/test 门禁通过，而不是测试数量或断言字面完全一致。

### P2-01 pseudo-vein runtime（✅ 2026-08-30）

- **范围与落点**：仅迁移 `server/src/world/pseudo_vein_runtime.rs` 原 `#[cfg(test)] mod tests` 的 20 个测试，外置到 `server/tests/unit/world/pseudo_vein_runtime_test.rs`；通过 `server/Cargo.toml` 的显式 `[[test]]` target 由 Cargo 原生发现。`tsy_container_search` 和其它模块未触及。
- **行为对拍**：迁移前 `../scripts/build-token.sh cargo test pseudo_vein_runtime::tests --lib` 为 `20 passed; 0 failed`；迁移后 `../scripts/build-token.sh cargo test --test pseudo_vein_runtime_unit` 为 `20 passed; 0 failed`。测试名、fixture、tick/数值、随机性（本模块无随机种子）和失败断言语义保持不变。
- **最小 seam**：仅为外置 integration test 暴露 `#[doc(hidden)]` 的生命周期常量、`set_test_state`、fallback baseline 构造器、既有 `round3` 纯函数及 narration/VFX/throttle 纯行为 helper；不扩大模块整体可见性、不复制生产实现、不改变 gameplay 路径。原因与清理边界在 PR body 登记：这些 seam 仅服务于本模块外置测试，后续若稳定 public contract 形成，应在 Test Refactor P4 收口时评估删除或收窄。
- **提交证据**：代码/测试/Cargo target 为 `5a0196e1a`（2026-08-30），validator finding 修复为 `3723db539`（2026-08-30）；最终 server 门禁 `fmt --check`、`clippy --all-targets -- -D warnings`、`cargo test` 均 exit 0，库测试 `12760 passed / 0 failed / 2 ignored`，外置 target `20 passed / 0 failed`。本条仅记录 P2 首批进度，P2 其它模块、P3、P4 仍未完成。

### P2-02 tsy_container_search（✅ 2026-08-30）

- **范围与落点**：仅迁移 `server/src/world/tsy_container_search.rs` 原 `#[cfg(test)] mod tests` 至 `server/tests/unit/world/tsy_container_search_test.rs`，由 `server/Cargo.toml` 的显式 `[[test]] name = "tsy_container_search_unit"` target 交给 Cargo 原生发现；生产模块不再保留该测试模块，未触及 pseudo-vein 或其它模块。
- **迁移前后对拍**：迁移前 `../scripts/build-token.sh cargo test tsy_container_search::tests --lib` 为 `28 passed / 0 failed / 0 ignored`；迁移后 `../scripts/build-token.sh cargo test --test tsy_container_search_unit` 为 `28 passed / 0 failed / 0 ignored`。测试名、fixture、随机种子、tick/时间、常量、断言与失败语义保持不变。
- **源码清单校正**：任务卡所述“29 个行为测试 + 7 个 helper”与 `origin/main=98a016785` 实际不符；迁移前逐项核验为 `28` 个 `#[test]` 与 7 个 helper（`make_inv`、`key_item`、`place_test_loot`、`placed_item_summaries`、`spirit_item`、`run_start_search_at_distance`、`collect_search_aborted`），未凭空新增第 29 个测试。
- **最小 seam**：仅将 `is_in_combat`、`damaged_this_tick`、`find_key_in_inventory`、`place_loot_in_carried_inventory` 暴露为 `#[doc(hidden)] pub`，供外置 integration test 调用；四者均为已有纯/非 gameplay helper，未扩大模块整体可见性、未复制生产实现、未改变 live system。该 seam 原因、范围与清理边界登记至本 PR：若 P4 收口后形成稳定公共契约，则评估删除或进一步收窄。
- **提交证据**：代码/测试/Cargo target 为 `8ed00ed31`（2026-08-30）；本条只记录 P2-02 迁移进度，P2 其它模块、P3、P4 仍未完成。

### P2-03 lingtian range_gate（✅ 2026-08-31）

- **范围与落点**：仅迁移 `server/src/lingtian/range_gate.rs` 原 `#[cfg(test)] mod tests` 的全部 9 个测试及 `validate_from_world`、`target` 两个必要 helper 至 `server/tests/unit/lingtian/range_gate_test.rs`；新增 `server/Cargo.toml` 的显式 `range_gate_unit` target。生产文件删除内联测试体，保留 `systems.rs` 现有测试依赖的 cfg(test) denial-log 支撑，未改变生产逻辑。
- **迁移前后对拍**：迁移前 `flock /tmp/bong-cargo.lock ../scripts/build-token.sh cargo test lingtian::range_gate::tests --lib` 为 `9 passed / 0 failed / 0 ignored`；迁移后 `flock /tmp/bong-cargo.lock ../scripts/build-token.sh cargo test --test range_gate_unit` 为 `9 passed / 0 failed / 0 ignored`。9 个原测试名、断言、边界/错误分支和失败语义保持不变。
- **完整 gate**：`flock /tmp/bong-cargo.lock bash -lc '../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library `12725 passed / 0 failed / 2 ignored`，bin `18 passed / 0 failed`，外置 `range_gate_unit` `9 passed / 0 failed`，其它 integration targets 与 doc-tests 无失败。
- **提交证据**：本 P2-03 代码、外置测试、Cargo target 与 evidence 更新由本分支提交；P2 其它模块、P3、P4 仍未完成。

### P2-04 craft recipe（✅ 2026-09-01）

- **范围与落点**：仅将 `server/src/craft/recipe.rs` 原 `#[cfg(test)]` 内的 28 个 `#[test]` 与最小 fixture/helper 迁移至 `server/tests/unit/craft/recipe_test.rs`；新增 `server/Cargo.toml` 的显式 `craft_recipe_unit` test target。测试保留原测试名、边界、错误分支和断言语义，使用 `bong_server::craft::recipe` 公开 API，未复制生产实现；生产行为、依赖、schema、wire、Redis、qi 及其它模块未改动。
- **迁移前后对拍**：迁移前 `cd server && ../scripts/build-token.sh cargo test craft::recipe::tests` 为 `28 passed / 0 failed`；迁移后 `cd server && ../scripts/build-token.sh cargo test --test craft_recipe_unit` 为 `28 passed / 0 failed`。
- **完整 gate**：server fmt check 通过；clippy `--all-targets -- -D warnings` 通过；`cargo test` 通过，library 为 `12697 passed / 0 failed / 2 ignored`，bin 为 `18 passed / 0 failed`，`craft_recipe_unit` 为 `28 passed / 0 failed`，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试、`craft_recipe_unit` target 与本条 evidence 对应 commit 为 `a19dddf11c69653f0d2f4bd62671af28149d8100`；PR #2137 已合并到 `4fa2b9af5a8a31ad56f3f7fc605b52892d743903`。P2 其它模块、P3、P4 仍未完成。

### P2-05 炼丹炉（✅ 2026-09-01）

- **范围与落点**：仅将 `server/src/alchemy/furnace.rs` 原 `#[cfg(test)] mod tests` 的全部 7 个 `#[test]` 外置到 `server/tests/unit/alchemy/furnace_test.rs`，并新增 `server/Cargo.toml` 的显式 `alchemy_furnace_unit` test target；生产文件删除内联测试体，运行时代码未改动。原测试没有独立 helper，测试内的 `AlchemySession` 与 `BlockPos` fixture 构造均按原语义保留。
- **迁移前后对拍**：迁移前 `cd server && ../scripts/build-token.sh cargo test alchemy::furnace::tests` 为 `7 passed / 0 failed`；迁移后 `cd server && ../scripts/build-token.sh cargo test --test alchemy_furnace_unit` 为 `7 passed / 0 failed`。7 个原测试名、边界/错误分支、时间/随机性和断言语义保持不变。
- **公开 API 与 seam**：外置测试仅使用 `bong_server::alchemy` 的公开 `AlchemyFurnace`、`AlchemySession`、`furnace_tier_from_item_id` 以及 Valence `BlockPos`；无需新增 doc-hidden seam、扩大可见性或复制生产实现。
- **完整 gate**：`flock /tmp/bong-cargo.lock -c 'cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'` 通过；library 为 `12690 passed / 0 failed / 2 ignored`，bin 为 `18 passed / 0 failed`，`alchemy_furnace_unit` 为 `7 passed / 0 failed`，其余 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试与 `alchemy_furnace_unit` target 对应 commit 为 `f11d0f430e8e48e91be575a33aa5cca3ea68f191`（2026-09-01）。本条仅记录 P2-05 进度；P2 其它模块、P3、P4 仍未完成。

### P2-06 processed_input（✅ 2026-09-01）

- **范围与落点**：仅将 `server/src/alchemy/processed_input.rs` 原 `#[cfg(test)] mod tests` 的全部 3 个 `#[test]` 外置到 `server/tests/unit/alchemy/processed_input_test.rs`，并新增 `server/Cargo.toml` 的显式 `processed_input_unit` test target；生产文件删除内联测试体，运行时代码未改动。原测试名、断言、边界和错误语义均保留。
- **迁移前后对拍**：迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test alchemy::processed_input::tests --lib'` 为 `3 passed / 0 failed / 0 ignored`；迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test processed_input_unit'` 为 `3 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅使用 `bong_server::alchemy` 公开 API；未复制生产实现，未新增测试 seam，未扩大无关可见性，未改变 wire、Redis、时间、随机性或玩法行为。
- **完整 gate**：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12687 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`processed_input_unit` 为 `3 passed / 0 failed`，doc-tests 为 `3 passed / 0 failed / 5 ignored`，其它 integration targets 无失败。
- **提交证据**：代码、外置测试、`processed_input_unit` target 对应 commit 为 `b765cc1a81bbb60b1871d068fdb5417fefc1eb82`（2026-09-01）。本条仅记录 P2-06 进度；P2 其它模块、P3、P4 仍未完成。

### P2-07 server readiness（✅ 2026-09-02）

- **范围与落点**：仅将 `server/src/server_readiness.rs` 原 `#[cfg(test)] mod tests` 的 5 个 `#[test]` 与 `TestDir` fixture 外置到 `server/tests/unit/server_readiness_test.rs`，并新增 `server/Cargo.toml` 的显式 `server_readiness_unit` target；生产文件删除内联测试体，readiness 运行时实现未改动。
- **行为对拍**：`server_readiness_unit` 为 `5 passed / 0 failed / 0 ignored`，保留原测试名、断言语义和临时目录 Drop 清理；覆盖精确 `pid=<pid>\n` 行、Unix `0600`、临时文件清理、目标已存在时 `AlreadyExists` 且不覆盖、并发一胜一败且败方为 `AlreadyExists`、临时文件名冲突重试并保留外部文件，以及无 filename 路径拒绝。
- **最小 seam**：仅将既有生产 `publish` 标为 `#[doc(hidden)] pub`，作为外置测试调用 seam；未暴露 `publish_created_temporary` 或 `TestDir`，未扩大无关 API，未复制生产实现。
- **完整 gate**：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12674 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`server_readiness_unit` 为 `5 passed / 0 failed`，doc-tests 为 `3 passed / 0 failed / 5 ignored`，其它 integration targets 无失败。
- **提交证据**：代码、外置测试、`server_readiness_unit` target 对应 commit 为 `5fe3b8605`（2026-09-02）。本条仅记录 P2-07 进度；P2 其它模块、P3、P4 仍未完成。

### P2-08 skill runtime（✅ 2026-09-02）

- **范围与落点**：仅将 `server/src/skill/mod.rs` 原 `#[cfg(test)] mod tests` 中实际存在的 7 个 skill runtime 测试外置到 `server/tests/unit/skill/runtime_test.rs`，并新增 `server/Cargo.toml` 的显式 `skill_runtime_unit` target；其中包含任务卡列出的 4 个核心行为测试及同模块已有的 3 个 runtime 回归测试。`skill/components.rs`、`skill/config.rs`、`skill/curve.rs`、`skill/events.rs` 的 inline tests 未迁移，生产 runtime、XP 公式、境界、wire、schema、Redis、client、AV、qi ledger 均未改动。
- **行为与定向验证**：保留 `register_adds_all_four_events`、`xp_above_cap_is_scaled_down_to_thirty_percent`、`xp_below_cap_is_not_scaled`、`record_skill_lv_up_appends_milestone` 及其余原测试名、fixture、tick/数值和断言语义；`register` 仍注册 `SkillXpGain`、`SkillLvUp`、`SkillCapChanged`、`SkillScrollUsed`，上限以上 XP 仍按既有规则缩放为 30%，上限以内不缩放，升级记录仍由 `record_skill_lv_up` 写入既有 narration 与 milestone 字段。`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test skill_runtime_unit'`：`7 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅调用已有运行时公开符号 `bong_server::skill::{register, consume_skill_xp_gain, record_skill_lv_up}` 及既有数据类型；未新增 `#[doc(hidden)]` seam，`default_skill_lv_up_narration` 生产 helper 保持私有且实现未复制到测试，`server/src/skill/mod.rs` 不再包含 inline test body。
- **完整 gate**：任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12672 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`skill_runtime_unit` 为 `7 passed / 0 failed`，其余 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试与 `skill_runtime_unit` target 对应 commit 为 `17ee3cf81`（2026-09-02）。本条仅记录 P2-08 进度；P2 总体、P3、P4 仍未完成。

### P2-09 world dimension（✅ 2026-09-03）

- **范围与落点**：仅将 `server/src/world/dimension.rs` 原 `#[cfg(test)] mod tests` 的全部 6 个 `#[test]` 外置到 `server/tests/unit/world/dimension_test.rs`，并新增 `server/Cargo.toml` 的显式 `dimension_unit` target；生产文件删除本模块测试体，保留其它 crate 内测试依赖的既有 `mark_test_layer_as_overworld` 最小 test seam。`DimensionKind`、`CurrentDimension`、`DimensionLayers`、TSY 注册配置及其它 gameplay、wire、Redis、schema 均未改动。
- **迁移前后对拍**：迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test world::dimension::tests --lib'` 为 `6 passed / 0 failed / 0 ignored`；迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test dimension_unit'` 为 `6 passed / 0 failed / 0 ignored`。测试名、fixture、断言语义保持不变。
- **公开 API 与 seam**：外置测试仅调用 `bong_server::world::dimension` 已有公开 API 与 Valence registry 类型，未复制生产实现、未新增 public seam；保留的 `mark_test_layer_as_overworld` 仅因其它尚未迁移的 crate 内测试仍依赖它，未扩大可见性或改变运行时行为。
- **最新主线合并后复验**：基于 `origin/main=c52c7933e` 执行 `git fetch origin && git merge origin/main`，无冲突并生成 merge commit `d36f98757`；合并后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test dimension_unit'` 仍为 `6 passed / 0 failed / 0 ignored`。
- **完整 gate**：合并后重新执行任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'`，exit 0；library 为 `12650 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`dimension_unit` 为 `6 passed / 0 failed / 0 ignored`，其它 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试与 `dimension_unit` target 对应 `e9bd7f11c`（2026-09-03）；格式修正对应 `8a15b805c`（2026-09-03）；本轮合并后复验对应 `d36f98757`（2026-09-03）。本条仅记录 P2-09 进度；P2 总体、P3、P4 仍未完成。

### P2-10 rift_portal（✅ 2026-09-03）

- **范围与落点**：仅将 `server/src/world/rift_portal.rs` 原 `#[cfg(test)] mod tests` 的全部 3 个测试外置到 `server/tests/unit/world/rift_portal_test.rs`，并新增 `server/Cargo.toml` 的显式 `rift_portal_unit` target；生产文件不再包含该测试体，运行时代码、fixture 内容和其它测试路径未改动。
- **行为与定向验证**：保留 `rift_kind_extract_table_matches_worldview`、`rift_kind_entry_exit_permissions`、`default_tsy_portals_fixture_loads` 原测试名、fixture、断言和行为；`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test rift_portal_unit'`：`3 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅调用已有公开 API `RiftKind::{base_extract_ticks, allows_entry, allows_exit}`、`load_tsy_portals_from_path` 及公开 registry 数据；无需新增 public/test seam，未复制生产逻辑，未扩大无关可见性。
- **完整 gate**：任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12672 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`rift_portal_unit` 为 `3 passed / 0 failed`，其它 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试与 `rift_portal_unit` target 对应 commit 为 `e170e26e1`（2026-09-03）。本条仅记录 P2-10 进度；P2 总体、P3、P4 仍未完成。

### P2-11 environment overlay（✅ 2026-09-03）

- **范围与落点**：仅将 `server/src/world/environment_overlay.rs` 原 `#[cfg(test)] mod tests` 的 9 个测试及 `zone_at`、`overworld_zone`、`spawn_default` 三个必要 fixture/helper 外置到 `server/tests/unit/world/environment_overlay_test.rs`，并新增 `server/Cargo.toml` 的显式 `environment_overlay_unit` target。生产文件删除全部 inline test body；未改动态雾堤运行时、AABB 相交规则、wire、schema、Redis、client、AV、qi ledger 或其它模块。
- **迁移对拍与行为**：保留 9 个原测试名、fixture、AABB 坐标、density/tick 数值和断言语义，覆盖递增 id、反转 AABB 归一化、非有限 density 防御性归零、FogVeil 字段映射、dimension 过滤、闭区间贴边相交、寿命到期/常驻和 remove/clear 行为。`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test environment_overlay_unit'`：`9 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅调用既有公开 `bong_server::world::environment_overlay::{EnvironmentOverlays, DEFAULT_FOG_BANK_TINT}`、`bong_server::world::environment::EnvironmentEffect`、`DimensionKind` 与 `Zone` 字段；未新增 `#[doc(hidden)]` seam，未复制 `aabb_overlaps` 或其它生产实现，未扩大无关 API。
- **完整 gate**：任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12664 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`environment_overlay_unit` 为 `9 passed / 0 failed`，doc-tests 为 `3 passed / 0 failed / 5 ignored`，其它 integration targets 无失败。
- **提交证据**：代码、外置测试与 `environment_overlay_unit` target 对应 commit 为 `4bc898e0d`（2026-09-03）。本条仅记录 P2-11 进度；P2 总体、P3、P4 仍未完成。

### P2-12 shader state（✅ 2026-09-03）

- **范围与落点**：仅将 `server/src/shader/mod.rs` 原 `#[cfg(test)] mod tests` 的全部 7 个测试外置到 `server/tests/unit/shader/state_test.rs`，并新增 `server/Cargo.toml` 的显式 `shader_state_unit` target；生产文件删除 inline test body，`shader_state_fields!` 宏、`ShaderStatePayload` 字段、`FIELD_NAMES`、JSON 序列化和 shader/client 行为均未改动。
- **迁移对拍与行为**：保留 `default_all_zeros`、`serializes_to_valid_json`、`field_mut_all_known`、`field_mut_unknown_returns_none`、`field_names_count_matches_struct`、`field_mut_write_read_round_trip`、`deserializes_from_json` 原测试名、断言、边界和错误语义；迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test shader::tests --lib'` 为 `7 passed / 0 failed / 0 ignored`（`12651 filtered out`），迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test shader_state_unit'` 为 `7 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅使用既有公开 `bong_server::shader::ShaderStatePayload` 及其 `Default`、`to_json_bytes`、`field_mut`、`FIELD_NAMES` API；未新增 seam、扩大可见性或复制生产实现。
- **完整 gate**：任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12649 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`shader_state_unit` 为 `7 passed / 0 failed / 0 ignored`，doc-tests 为 `3 passed / 0 failed / 5 ignored`，其它 integration targets 无失败。
- **提交证据**：代码、外置测试与 `shader_state_unit` target 对应 commit 为 `f8c4bc9ff`（2026-09-03）。本条仅记录 P2-12 进度；P2 总体、P3、P4 仍未完成。
### P2-13 skin packet（✅ 2026-09-03）

- **范围与落点**：仅将 `server/src/skin/packet.rs` 原 `#[cfg(test)] mod tests` 的 2 个测试及 `test_skin` helper 外置到 `server/tests/unit/skin/packet_test.rs`，并新增 `server/Cargo.toml` 的显式 `skin_packet_unit` target；生产文件删除 inline test body，未改 `NpcPlayerInfoUpdateS2c`/`NpcPlayerInfoRemoveS2c` 运行时编码、MC 1.20.1 packet ID、skin 协议或其它模块。
- **行为与协议对拍**：保留 `player_info_packet_matches_protocol_field_order`、`player_info_packet_id_is_mc_1_20_1_player_list_update` 原测试名、`test_skin` fixture、完整协议字节断言、编码字段顺序、packet ID 与错误语义；迁移前 `skin::packet::tests` 为 `2 passed / 0 failed / 0 ignored`，迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test skin_packet_unit'` 为 `2 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅调用已有公开 `NpcPlayerInfoUpdateS2c`/`NpcPlayerInfoRemoveS2c` 的 `Encode`、`Packet::ID`、`SignedSkin`/`SkinSource` 与 Valence packet IDs；未新增 seam，未复制生产实现，未扩大无关可见性。
- **完整 gate**：主线合并后按提升权限 locked 命令 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library 为 `12654 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`skin_packet_unit` 为 `2 passed / 0 failed`，其它 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码、外置测试与 `skin_packet_unit` target 对应 commit 为 `b28d41c09`（2026-09-03）；本条仅记录 P2-13 进度，P2 总体、P3、P4 仍未完成。

### P2-15 forge history（✅ 2026-09-04）

- **范围与落点**：仅将 `server/src/forge/history.rs` 原有 `#[cfg(test)]` 模块中的 2 个测试外置到 `server/tests/unit/forge/history_test.rs`，并新增 `server/Cargo.toml` 的显式 `forge_history_unit` target；生产文件不再包含该测试体。`ForgeAttempt::from_bucket` 的 bucket 映射、`ForgeHistory::recent` 的尾部语义、forge gameplay、schema、wire、qi 及其它模块均未改动。
- **迁移对拍与行为**：迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test forge::history::tests --lib'` 为 `2 passed / 0 failed / 0 ignored`；迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test forge_history_unit'` 为 `2 passed / 0 failed / 0 ignored`。`bucket_tag_mapping` 与 `recent_tails_n_entries` 的测试名、fixture、断言和行为保持不变。
- **公开 API 与 seam**：外置测试仅使用既有公开 API `bong_server::forge::history::{ForgeAttempt, ForgeHistory}` 与 `bong_server::forge::events::ForgeBucket`；未复制生产逻辑、未新增或扩大可见性 seam。
- **最新主线与完整 gate**：提交前执行 `git fetch origin && git merge origin/main`，`origin/main` 从 `79d953416` 更新至 `fdabe0f0b` 并 fast-forward 合并；主线已有 `shader_state_unit` target 及 plan 条目均保留。最终收口前再次执行 `git fetch origin && git merge origin/main`，将 `origin/main` 从 `fdabe0f0b` 更新至 `1cfec210d`，仅带入 client UI 与其 plan 变更，并形成合并提交 `f46b69bee`；未触及本任务 Cargo/forge 文件。最新一次按任务卡执行 `git fetch origin && git merge origin/main` 将 `origin/main` 更新至 `c25f22762`，在 `docs/plan-test-layout-refactor-v1.md` 发生冲突时保留 P2-14 与 P2-15 全部 evidence，形成中文合并提交 `391fb2431`；主线带入的 `recipe_fragment_unit` Cargo target、生产测试迁移和外置测试均保留。合并后 slot-3 通过 `forge_history_unit`（`2 passed / 0 failed`）与 `recipe_fragment_unit`（`3 passed / 0 failed`），再执行任务卡指定的 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` 并通过：library `12636 passed / 0 failed / 2 ignored`，main binary `18 passed / 0 failed`，`forge_history_unit` `2 passed / 0 failed`，`recipe_fragment_unit` `3 passed / 0 failed`，其它 targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`。
- **提交证据**：forge history 外置代码与 `forge_history_unit` target 对应 commit 为 `9ba197724`（2026-09-04）；本条仅记录 P2-15 进度，P2 其它模块、P3、P4 仍未完成。
### P2-14 alchemy recipe_fragment（✅ 2026-09-04）

- **范围与落点**：仅将 `server/src/alchemy/recipe_fragment.rs` 原有的 3 个 `#[test]` 及 `recipe_with_stage_count` fixture 外置到 `server/tests/unit/alchemy/recipe_fragment_test.rs`，并新增 `server/Cargo.toml` 的显式 `recipe_fragment_unit` target；生产文件不再包含本模块测试体，炼丹运行时、schema、wire、Redis、qi 及其它模块未改动。
- **迁移对拍与行为**：迁移前 `alchemy::recipe_fragment::tests` 为 `3 passed / 0 failed / 0 ignored`，迁移后 `recipe_fragment_unit` 为 `3 passed / 0 failed / 0 ignored`。保留 `normalized_fragment_drops_unknown_stage_and_clamps_quality`、`fragment_with_at_least_half_stages_keeps_partial_quality_cap`、`fragment_below_half_stages_is_capped_to_tier_one` 原测试名、边界、断言、recipe fixture 语义和 quality/stage 规则。
- **公开 API 与 seam**：外置测试仅调用 `bong_server::alchemy::recipe` 与 `bong_server::alchemy::recipe_fragment` 的公开 API；未新增 seam、扩大可见性或复制生产实现。
- **完整 gate**：基于 `origin/main=fdabe0f0b528815472f1660fb08a2486c8762fd1` 合并后，在本 slot 执行 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'`，exit 0；fmt、clippy 均通过，library 为 `12638 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`recipe_fragment_unit` 为 `3 passed / 0 failed / 0 ignored`，其它 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：P2-14 代码、外置测试与 `recipe_fragment_unit` target 对应实现 commit 为 `5bd05a6839abbb99287a036b91eafde191a68b7e`；合并最新主线的 commit 为 `e89e5519cd88c2ea9f76cdec1ee229b46d1dc012`。主线合并后保留全部既有 P2-01～P2-13 条目及其它 Cargo test targets；本条为独立 P2-14 evidence，P2 总体、P3、P4 仍未完成。

### P2-16 schema client payload（✅ 2026-09-04）

- **范围与落点**：仅将 `server/src/schema/client_payload.rs` 原 `#[cfg(test)] mod tests` 的全部 9 个测试（7 个 sample 反序列化、1 个全样本 roundtrip、1 个 `ClientPayloadType` literals）外置到 `server/tests/unit/schema/client_payload_test.rs`，并新增 `server/Cargo.toml` 的显式 `client_payload_unit` target；生产 schema、TypeBox、samples、wire、Redis、玩法与其它迁移范围未改动。
- **迁移对拍与行为**：保留 `deserialize_welcome_sample`、`deserialize_heartbeat_sample`、`deserialize_narration_sample`、`deserialize_zone_info_sample`、`deserialize_event_alert_sample`、`deserialize_locust_swarm_warning_sample`、`deserialize_player_state_sample`、`roundtrip_all_client_payload_samples`、`deserialize_client_payload_type_literals` 原测试名、`include_str!` sample 路径、serde `tag`/`rename_all` 语义、断言与错误语义；迁移前 `schema::client_payload::tests` 为 `9 passed / 0 failed / 0 ignored`，迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test client_payload_unit'` 为 `9 passed / 0 failed / 0 ignored`。
- **公开 API 与 seam**：外置测试仅调用既有公开 `ClientPayloadV1`、`ClientPayloadType`、`EventAlertSeverity`、`EventKind` 及 serde API；未复制生产实现、未新增 seam、未扩大可见性。
- **主线合并与完整 gate**：基于 `origin/main=fdabe0f0b528815472f1660fb08a2486c8762fd1` 执行 `git fetch origin && git merge origin/main`，无冲突并生成 merge commit `bb9f1b78d`；格式修正 commit 为 `2b8c4f1aa`。修正后定向测试仍为 `9 passed / 0 failed / 0 ignored`；`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0，library 为 `12632 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`client_payload_unit` 为 `9 passed / 0 failed`，其它独立 Cargo targets 与 doc-tests 均无失败。
- **提交证据**：P2-16 代码、`client_payload_unit` target 与测试迁移对应 `699e5b545`（2026-09-03）；主线合并对应 `bb9f1b78d`，格式修正对应 `2b8c4f1aa`。本条仅记录 P2-16 进度，P2 总体、P3、P4 仍未完成。
- **最终收口证据**：最终提交前在 slot-1 执行 `git fetch origin --prune && git merge origin/main`，确认最新 `origin/main=1cfec210df91357eb16329ca8ce6ae71a651ebdf` 已由 `404151a82` 合入且无待合并变更；随后同一 slot 的提升权限 locked server gate exit 0，library 为 `12632 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed`，`client_payload_unit` 为 `9 passed / 0 failed`，全部既有独立 Cargo targets 与 doc-tests 均通过。
- **最新主线收口证据**：本次最终提交前在 slot-1 执行 `git fetch origin --prune && git merge origin/main`，将最新 `origin/main=dd7c63107496050f1e9bb937a624477525b64022` 合入并形成合并提交 `a6e96851c8fae239098606a1706e698755055504`；合并带入 P2-15 的 `forge_history_unit` target、外置测试及 plan evidence，同时保留 P2-01～P2-16（含 P2-12 shader）证据和全部既有 Cargo test targets。因触及 `server/Cargo.toml`、server 测试和 active plan，随后同一 slot 执行提升权限 locked server gate，exit 0：library `12627 passed / 0 failed / 2 ignored`，main binary `18 passed / 0 failed`，`client_payload_unit` `9 passed / 0 failed`，`recipe_fragment_unit` `3 passed / 0 failed`，`shader_state_unit` `7 passed / 0 failed`，`skin_packet_unit` `2 passed / 0 failed`，`forge_history_unit` `2 passed / 0 failed`，其余 integration targets 与 doc-tests（`3 passed / 0 failed / 5 ignored`）均通过。

### P2-17 inventory inline tests（✅ 2026-09-05）

- **分类与处置范围**：按 P2 准入策略逐条复核 `server/src/inventory/mod.rs` 原有 386 个 `#[test]`（P0 诊断为 14,102 个测试行、文件 20,699 行），并将 386 条记录写入 `docs/inline-test-inventory.tsv`。40 个可由既有公开 API 驱动的 allocator/free-slot/grant/merge/move 契约测试外置到 `server/tests/unit/inventory/inventory_test.rs`，新增显式 `inventory_unit` target；其余 346 个依赖私有解析、ECS 装配或仍需契约收缩的测试保留在独立的 `server/src/inventory/tests.rs`，不新增测试 seam。`mod.rs` 现为 6,710 行，仅保留 `#[cfg(test)] mod tests;` 声明，生产实现、库存事务、容量/权限、ItemCategory、模板加载与 qi 语义未改动。
- **迁移对拍与完整性**：迁移前基线为 inventory 386 条测试通过；迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test inventory::tests --lib'` 为 `374 passed / 0 failed / 0 ignored`（inventory 私有测试 346 条，另含既有 `schema::inventory` 28 条），外置 `inventory_unit` 定向测试为 40 条通过；原 386 个测试名在 `tests.rs` 与 `inventory_unit` 的并集中逐一对拍，无测试名丢失。原测试断言、边界/错误语义和涉及真元的守恒断言均保留；仅移除已无调用的测试辅助函数 `assert_container_has_no_overlaps`。
- **公开 API 与 seam**：外置测试仅使用已有 `bong_server::inventory` 公开类型/函数、`bong_server::world::dimension::DimensionKind` 与测试 fixture；未复制生产实现，未新增或扩大 `pub`/`#[doc(hidden)]` seam。私有测试继续在模块测试上下文中验证私有解析和 ECS 装配，不为外置而改变生产可见性。
- **提交证据**：逐条分类清单对应 `476a98576`（2026-09-05），40 条外置与 `inventory_unit` target 对应 `8c69a171f`，fixture/清单收口对应 `562d0b702`；每个提交均带 `Model: gpt-5.6-luna`。本条仅记录 P2-17 本批次；后续 inventory 契约收缩与其它模块仍属 P4 范围。
### P2-17 schema proto_gen 协议 pin（✅ 2026-09-05）

- **范围与落点**：按 `docs/inline-test-inventory.tsv` 当前 HEAD 的 382 条 `schema/proto_gen.rs` 逐条分类，将生产文件收缩为 11 行 `pub mod bong { include!(...) }` 入口；测试外置到 `server/tests/unit/schema/proto_gen_test.rs`，并新增显式 `proto_gen_unit` Cargo target。未改 `proto/*.proto`、`build.rs`、生成链、schema/wire、client、agent 或玩法。
- **契约处置**：382 条中 54 条保留协议枚举、错误字节、兼容演进和外部 payload 结构 pin；328 条同构 roundtrip 由一个表驱动 `merged_protocol_pin_cases` runner 调用并以测试名报告失败；删除 0 条。每个减少项均对应 TSV 的“合并”处置，未新增 `pub`、`#[doc(hidden)]` 或其它测试 seam，也未复制生产实现。
- **迁移对拍**：迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test schema::proto_gen::tests --lib'` 为 `381 passed / 0 failed / 1 ignored`；迁移后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test proto_gen_unit'` 为 `55 passed / 0 failed / 0 ignored`。原 ignored benchmark 已作为 TSV 合并 case 纳入 runner，未再静默跳过。
- **完整门禁**：当前 HEAD 执行任务卡指定的提升权限 locked 命令 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit 0；library、main binary、全部独立 Cargo targets、integration targets 与 doc-tests 均无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交证据**：代码与 target 对应 `461646f15`（2026-09-05），runner 格式修正对应 `74323058f`（2026-09-05）；本条仅记录 P2-17 进度，P2 总体、P3、P4 仍未完成。

### P2-18 lingtian systems（✅ 2026-09-08）

- **范围与落点**：仅将 `server/src/lingtian/systems.rs` 原 `#[cfg(test)] mod tests` 的全部 98 个测试外置到同 crate 文件 `server/src/lingtian/systems_tests.rs`，生产文件末尾仅保留 `#[cfg(test)] #[path = "systems_tests.rs"] mod tests;` 挂载；未新增 Cargo test target。
- **迁移对拍与行为**：迁移前、后均使用 `cd server && ../scripts/build-token.sh cargo test --lib lingtian::systems::tests`，分别为 `98 passed / 0 failed / 0 ignored`；测试名序列 98/98 相同，fixture、断言、错误语义、事件形状与 system 注册顺序未改。
- **同 crate 必要性与保留项**：本批只能使用同 crate 外置，因为测试依赖既有 `#[cfg(test)] pub(crate) struct StartHandlerPlotScanCount`、`#[cfg(test)] fn build_start_plot_index`，以及五处生产 system 形参上的 `#[cfg(test)] mut plot_scan_count: Option<ResMut<StartHandlerPlotScanCount>>`（约原 L567/L689/L748/L790/L856）和对应 cfg(test) 调用分支。上述 Resource、helper、形参与 cfg(test) 门均原样保留，未迁移、未去掉、未改可见性；这些既有设计不属于本 PR 的 seam 清理范围，后续如需整改另行立项。
- **源码与可见性核验**：迁移前已核对 `systems.rs` 无 `include_str!` 源码文本消费者；迁移后生产文件无测试体，未复制生产实现，未新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam；未改 qi_physics、玩法、事件或 system 注册顺序。
- **提交证据**：代码迁移对应 `aa615d801`（2026-09-08），本条仅记录 P2-18 进度；P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-19 cultivation tribulation（✅ 2026-09-07）

- **范围与落点**：仅处置 `server/src/cultivation/tribulation.rs` 原有 119 个 `#[test]`（测试体约 7,029 行；生产段约 4,173 行）。103 条能经既有公开 API 验证的 A 类测试外置到 `server/tests/unit/cultivation/tribulation_test.rs`，新增 `tribulation_unit` Cargo target；16 条 B 类测试保留在 `server/src/cultivation/tribulation_tests.rs`，由生产文件末尾的 `#[cfg(test)] #[path = "tribulation_tests.rs"] mod tests;` 挂载。渡劫运行时、schema、wire、Redis、qi 路径及其它模块未改动。
- **迁移对拍与行为**：迁移前 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test cultivation::tribulation::tests --lib'` 为 `119 passed / 0 failed / 0 ignored`；迁移后同 crate B 类为 `16 passed / 0 failed / 0 ignored`，`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test tribulation_unit'` 的 A 类为 `103 passed / 0 failed / 0 ignored`，逐位守恒 `103 + 16 = 119`。119 个原测试名、断言、边界、fixture 语义和随机/时间输入均保持不变；仅按外置边界改写 crate 路径为 `bong_server::`，未复制生产实现。
- **B 类逐条保留理由**：
  - `long_full_progress_du_xu_request_adds_heart_demon_and_kaitian_waves`：直接锁定私有 `DUXU_FULL_PROGRESS_MIN_TICKS`，外置会把内部进度阈值变成生产 API。
  - `recent_full_progress_du_xu_request_keeps_default_three_waves`：直接锁定同一私有完整进度阈值与内部波次选择，外置会暴露实现内部的时间窗口。
  - `full_progress_boundary_35999_keeps_three_waves`：同时读取私有阈值、默认波次数和 `du_xu_full_progress_ticks`/`du_xu_waves_total`，外置会把边界算法及默认值固化成 API。
  - `full_progress_boundary_36000_adds_five_waves`：验证私有阈值切换和内部波次计算，公开它会扭曲生产 API 的抽象边界。
  - `resolved_heart_demon_after_soul_devouring_skips_original_heart_demon_slot`：断言私有 `DUXU_KAITIAN_WAVE` 的内部 slot 跳转，外置会要求公开波次调度细节。
  - `heart_demon_obsession_timeout_penalizes_qi_and_boosts_kaitian_damage`：断言私有心魔后续波次倍率，外置会把平衡实现常数变成稳定 API。
  - `heart_demon_resolution_advances_to_kaitian_without_republishing_fourth_wave`：读取私有心魔后续波次倍率来锁定内部结算，公开会泄露非生产契约的倍率细节。
  - `obsession_resolution_increases_kaitian_damage`：同样直接锁定私有倍率而非外部结果接口，外置会迫使内部平衡表公开。
  - `phase7_balance_three_wave_curve_fits_spirit_pool`：直接调用私有 `du_xu_wave_profile`，验证内部 profile 曲线；公开它会把算法 profile 变成生产 API。
  - `aoe_uses_current_wave_strength_only_on_wave_start_tick`：读取私有链式雷击 strike 数，外置会暴露 AOE 内部实现计数而非稳定行为契约。
  - `spectator_aoe_is_not_reduced_by_distance_within_danger_radius`：同样依赖私有 strike 数来验证内部 AOE 展开，公开会扭曲 AOE 实现边界。
  - `third_wave_freezes_qi_max_as_soul_devouring_lightning`：断言私有 `DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO`，外置会将内部冻结比例作为公共 API。
  - `juebi_terrain_generation_keeps_animation_order`：依赖私有地形操作入队函数，外置需公开内部队列生成算法而非地形结果契约。
  - `juebi_terrain_tick_skips_unloaded_chunks_without_air_restore`：直接构造私有 `TerrainModOp`，外置会把内部地形操作结构公开，扭曲生产 API。
  - `juebi_terrain_tick_records_original_once_for_overlapping_ops`：同样验证私有 `TerrainModOp` 队列去重/原始状态记录，公开会固化内部存储结构。
  - `tribulation_omen_cloud_blocks_overlay_and_restore`：依赖仅在 `cfg(test)` 提供的 `mark_test_layer_as_overworld` 测试层装配钩子；外置需新增 test-only seam 才能重建相同 ChunkLayer fixture，故留同 crate。
- **公开 API 与 seam**：A 类只使用既有 `bong_server::cultivation::tribulation` 及相关公开模块 API；B 类使用同 crate 访问现有私有项。没有新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，也没有改动生产实现。
- **主线复验与完整门禁**：在初轮 gate 后紧邻执行 `git fetch origin && git merge origin/main`，基于 `origin/main=eedf9903d8f2d819847d4cf394e43ddf7bb2b0fa` 无 Cargo/plan 冲突，生成 merge HEAD `de21913542389e1e9a4f4a59a8c148c750450d95`；合入内容仅为 bot 场景脚本与已归档 bot plan，未触及本批 server、Cargo 或 tribulation 文件。合并后再次执行 locked server gate `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'`，exit 0：library `12071 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`、`tribulation_unit` `103 passed / 0 failed / 0 ignored`，其它既有 Cargo targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`；Cargo 其它 targets、P2/P3/P4 既有条目均保留。
- **validator 与提交证据**：无上下文只读 validator 已绑定代码/分类 HEAD `9b71f1fb9d67b252c28573ddb175c0666b3d31e5` 并 PASS；合并 HEAD 上再次定向对拍同 crate B 类 `16 passed / 0 failed / 0 ignored`、外置 A 类 `103 passed / 0 failed / 0 ignored`，逐位守恒 `103 + 16 = 119`。代码、`tribulation_unit` target 与测试落点对应 `3cf9ef551`（2026-09-07），分类 evidence 对应 `9b71f1fb9`，本段为本批中文 docs evidence commit；每个提交带 `Model: gpt-5.6-luna`。本条仅记录 P2-19 进度，P2 总体、P3 后续阶段、P4 仍未完成，plan 不归档。

### P2-21 network/mod（✅ 2026-09-08）

- **范围与落点**：仅将 `server/src/network/mod.rs` 原有单个 `#[cfg(test)] mod tests` 的全部 62 个 `#[test]` 及其测试 imports 外置到同 crate 文件 `server/src/network/mod_tests.rs`，由 `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;` 挂载；生产文件由 7,548 行收缩至 3,559 行，未改 network 运行时、payload 构造、schema、wire、client、agent 或其它测试模块。
- **迁移对拍与行为**：迁移前 `cd server && ../scripts/build-token.sh cargo test network::tests:: --lib` 为 `62 passed / 0 failed / 0 ignored`；迁移后同一命令为 `62 passed / 0 failed / 0 ignored`（真实 `POST_FIX_COMPARE_EXIT=0`）。62 个原测试名、fixture、断言、payload 期望、边界/错误语义与执行顺序保持不变；未新增 Cargo test target，沿用 network 已有同 crate 独立测试模块形态。
- **测试可见性与生产残留**：未新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，也未复制生产实现。`build_player_state_payload`、`collect_players_for_world_state` 两个既有 `#[cfg(test)] #[allow(dead_code)] pub(crate)` 辅助函数及 `anim_wiring_manifest_test`、`quickslot_config_emit_test`、`skill_vfx_wiring_test`、`skillbar_config_emit_test` 四个既有测试模块声明均未改动；它们不属于本批 62 条搬迁，保留原位置与可见性，后续按 P4 清单单独审计。
- **提交证据**：代码与 `mod_tests.rs` 对应提交 `8ac457358`（2026-09-08），本条仅记录 P2-21 进度；P2 总体、P3 后续阶段、P4 仍未完成，plan 不归档。
- **源码接线自检判据修复**：`server/src/network/rat_av_trigger.rs` 与 `server/src/network/rat_qi_tier_emit.rs` 内既有 `production_source` 自检 helper 原先依赖字面 `#[cfg(test)]\nmod tests {` marker；第一版兼容修复改用 `#[path = "mod_tests.rs"]\nmod tests;` 截断，却因 `include_str!("mod.rs")` 读取的是完整源码而在挂载声明处提前丢弃后续文本，破坏接线 pin。最终两处只保留完整的非注释源码，不再按 inline/外置测试文本形状截断；`rat_av_trigger` 对 `brain_rat::register` 改用真实 `App` 事件资源接线核验。这样两种测试布局均不改变判据，且不再遗留旧 marker/fallback 双路径；不改回 `mod.rs` 结构、不动四个既有外置模块、玩法或生产运行时。
### P2-20 schema proto_convert（✅ 2026-09-08）

- **范围与落点**：仅处置 `server/src/schema/proto_convert.rs` 原有两个 `#[cfg(test)]` 模块（`race_gate_wire_tests` 7 条、`tests` 79 条），合计 86 条；77 条可由既有公开 proto 转换/生成类型验证的 A 类测试外置到 `server/tests/unit/schema/proto_convert_test.rs`，新增显式 `proto_convert_unit` target；9 条依赖私有纯转换 helper 的 B 类测试原样留在 `server/src/schema/proto_convert_tests.rs`，由生产文件末尾的 `#[cfg(test)] #[path = "proto_convert_tests.rs"] mod tests;` 挂载。生产文件从 8,448 行收缩至实际 4,275 行；未改 proto3 schema、build.rs、生成链、wire、client、agent、Redis、qi 或转换运行时逻辑。
- **迁移对拍与行为**：干净 `origin/main=ada8c48cd` 基线 `cd server && ../scripts/build-token.sh cargo test schema::proto_convert:: --lib` 为 `86 passed / 0 failed / 0 ignored`；迁移后无外层 flock 的 A 类 `cd server && ../scripts/build-token.sh cargo test --test proto_convert_unit` 为 `77 passed / 0 failed / 0 ignored`，B 类 `cd server && ../scripts/build-token.sh cargo test schema::proto_convert::tests --lib` 为 `9 passed / 0 failed / 0 ignored`，逐位守恒 `77 + 9 = 86`。所有原测试名、断言、fixture、proto encode/decode 字节与 uint64→字符串、enum→全名、坐标拍平、oneof/presence 语义保持不变。
- **B 类逐条保留理由**：
  - `cast_outcome_reject_race_mismatch_maps_to_dedicated_proto_variant`：直接锁定私有 `cast_outcome_to_proto` 的内部 enum→i32 映射；外置会迫使实现 helper 成为生产 API，而稳定契约应由外部 wire 结果承载。
  - `inventory_item_view_freshness_survives_proto_roundtrip_for_all_tracks`：直接调用私有 `inventory_item_view_to_proto` 验证 freshness 字段装配；外置会公开内部字段组装入口，而非只承诺 InventoryItemView wire。
  - `inventory_item_view_without_freshness_keeps_proto_field_absent`：锁定私有 converter 的 `Option` presence 组装；外置会把当前省略字段的内部实现固化成生产 API。
  - `inventory_item_view_pill_alchemy_survives_proto_roundtrip`：依赖私有 converter 的丹药 alchemy 嵌套映射；外置会暴露内部炼丹 item 到 proto 的装配细节，而不是稳定 wire 契约。
  - `inventory_item_view_recipe_fragment_alchemy_survives_proto_roundtrip`：依赖私有 converter 的 recipe-fragment 嵌套装配；公开它会固化内部残方结构与映射 helper 的 API 形状。
  - `inventory_item_view_recipe_hint_alchemy_survives_proto_roundtrip`：依赖私有 converter 的 recipe-hint 字段装配；外置会把内部提示映射实现误变成生产可见接口。
  - `inventory_item_view_pill_residue_alchemy_survives_proto_roundtrip`：依赖私有 residue 转换路径来锁定 wire presence 与 snake_case；外置会暴露内部炼丹 residue 类型映射，而非公开稳定转换契约。
  - `inventory_item_view_no_alchemy_absent_in_proto_wire`：依赖私有 converter 的 `None` presence 行为；外置会为测试开放内部缺省字段实现，扭曲生产 API 抽象边界。
  - `c2s_zhenfa_kind_proto_pins_include_trap_runtime_variants`：直接锁定私有 `zhenfa_kind_to_proto` 的内部 enum 映射；外置会公开实现 helper，而不是验证稳定的请求 wire 行为。
- **公开 API 与 seam**：A 类仅使用既有 `bong_server::schema::proto_convert`、schema 数据类型与 prost 生成类型；B 类使用同 crate 访问既有私有项。没有新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，没有复制生产实现。
- **验证与提交证据**：迁移前干净基线为 `86 passed / 0 failed / 0 ignored`；迁移后 A 类 `77 passed`、B 类 `9 passed`，逐位守恒 `77 + 9 = 86`；合并 HEAD `f0a0567bf` 上再次执行 A/B 定向对拍，仍分别为 `77 passed / 0 failed / 0 ignored` 与 `9 passed / 0 failed / 0 ignored`。合并前代码/分类提交为 `54c8c8e3f`，A 类 clippy 注释修正为 `c239956ee`，B 类 clippy 注释修正为 `37bc4bdd1`，紧邻 `git fetch origin && git merge origin/main` 合入 `origin/main=d9e1839e2`，无 Cargo/plan 冲突；合并后完整 server gate（无外层 flock，三条均由 `build-token.sh` 排队）fmt、clippy、test 均 exit 0，库测试 `12000 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`，全部 Cargo targets 与 doc-tests 通过（doc-tests `3 passed / 0 failed / 5 ignored`）。最终无上下文只读 validator 绑定本批最终 HEAD 并 PASS。每个提交带 `Model: gpt-5.6-luna`。本条仅记录 P2-20，P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-23 npc dormant（✅ 2026-09-08）

- **范围与落点**：仅处置 `server/src/npc/dormant/mod.rs` 原有单个 `#[cfg(test)] mod tests` 的 98 条测试（源文件 6,867 行，测试区约 4,344 行）。23 条能经既有公开 dormant/qi/zone/serde/combat API 验证的 A 类测试外置到 `server/tests/unit/npc/dormant_test.rs`，新增 `npc_dormant_unit` Cargo target；75 条依赖同 crate 私有系统、私有 seed/migration/parser、ECS/Redis/战斗装配或 test-only accessor 的 B 类测试原样留在 `server/src/npc/dormant/mod_tests.rs`，由生产文件末尾的 `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;` 挂载。未改 dormant 运行时、schema、wire、Redis、qi_physics 常量或 `release_dormant_qi_to_zone` 守恒路径。
- **迁移对拍与行为**：迁移前 `cd server && ../scripts/build-token.sh cargo test npc::dormant::tests:: --lib` 为 `98 passed / 0 failed / 0 ignored`；迁移后同 crate B 类为 `75 passed / 0 failed / 0 ignored`，`cd server && ../scripts/build-token.sh cargo test --test npc_dormant_unit` 的 A 类为 `23 passed / 0 failed / 0 ignored`，逐位守恒 `23 + 75 = 98`。98 个原测试名、断言、边界和 fixture 语义保持不变；A 类仅将测试 crate 路径改为 `bong_server::`，未复制生产实现。
- **B 类逐条保留理由**：以下每条均依赖同 crate 私有实现细节或无法由 integration crate 重建的测试装配；外置会迫使这些非稳定细节成为生产 API，故保留在 `mod_tests.rs`。
  - `startup_janitor_purges_leaked_tmp_keys`：依赖私有 Redis janitor 选择/分批 helper；外置会公开临时键清理算法，而非稳定的持久化结果契约。
  - `dormant_scatter_stays_in_zone_bounds`：直接锁定私有散布算法；外置会把 seed 几何实现固化成生产 API。
  - `dormant_scatter_spreads_across_zone_instead_of_clustering`：直接锁定私有散布覆盖率与间距算法；外置会暴露内部采样策略。
  - `dormant_scatter_is_deterministic_and_distinct`：直接锁定私有 seed 散布函数；外置会把内部碰撞/采样细节变成 API。
  - `store_indexes_by_archetype_and_zone`：依赖 `ids_by_archetype` 与 `ids_by_zone` 两个 `#[cfg(test)] pub fn`；它们没有 integration 生产契约，放宽可见性会新增 seam。
  - `dormant_store_dirty_set_on_seed_age_death`：依赖 seed/aging/death 私有装配和 dirty bookkeeping；外置会固化内部 mutator 顺序。
  - `dormant_receipt_cleanup_requires_current_success_and_keeps_tombstone_on_sqlite_failure`：依赖私有回执清理、tombstone 与 SQLite 失败时序；外置会公开内部提交编排。
  - `dormant_persistence_receipts_are_revision_safe`：依赖同 crate 的 revision receipt orchestration；外置会把内部 revision 状态机形状当成生产 API。
  - `dormant_store_dirty_per_mutator_and_clean_on_restore`：依赖私有 store mutator/restore bookkeeping；外置会固化内部 dirty 标志实现。
  - `daozhan_terminal_release_settles_both_external_owners_atomically`：依赖私有 Daozhan terminal 多 owner 结算装配；外置会暴露事务内部阶段而非可观察结果。
  - `dormant_global_tick_settles_expired_daozhan_cultivation_and_drain_owners`：依赖私有 global tick/ECS 装配；外置会把系统资源接线变成公共接口。
  - `dormant_global_tick_retains_expired_snapshot_when_settlement_fails`：依赖 global tick 的私有失败回滚路径；外置会固化内部失败传播顺序。
  - `dormant_global_tick_clears_indexes_when_all_snapshots_expire`：依赖私有 aging/expiration 与 index rebuild；外置会公开索引维护实现。
  - `dormant_global_tick_refreshes_zone_index_after_movement`：依赖私有 tick movement/index 刷新装配；外置会把内部索引更新时机变成 API。
  - `dormant_breakthrough_uses_cultivation_rules_below_duxu`：依赖私有 dormant breakthrough settlement wiring；外置会公开运行时内部转接层。
  - `dormant_breakthrough_settlement_failure_rolls_back_all_state`：依赖私有突破失败回滚装配；外置会把事务内部步骤固化为 API。
  - `loads_dormant_snapshots_from_redis_hash_entries`：依赖私有 Redis hash loader/批量 restore；外置会公开内部存储适配层。
  - `terminal_tombstone_suppresses_even_corrupt_stale_row_before_batch_validation`：依赖私有 tombstone 优先级和批量校验顺序；外置会扭曲持久化实现边界。
  - `redis_hash_restore_rejects_partial_corruption_without_mutating_live_store`：依赖私有 hash restore 事务装配；外置会把内部原子替换步骤变成生产 API。
  - `failed_restore_blocks_tombstone_cleanup_hash_replacement`：依赖私有 restore-failure gate 与 tombstone 清理流程；外置会暴露内部恢复状态机。
  - `redis_hash_restore_rejects_negative_daozhan_owner_atomically`：依赖私有嵌套 Daozhan owner 校验；外置会公开 Redis 反序列化内部校验器。
  - `redis_hash_restore_rejects_hash_field_identity_mismatch_atomically`：依赖私有 hash field identity 校验；外置会固化存储字段与校验实现。
  - `redis_hash_restore_rejects_every_nested_identity_mismatch_atomically`：依赖私有嵌套 identity validator；外置会公开内部快照结构细节。
  - `redis_hash_restore_rejects_semantically_invalid_cultivation_atomically`：依赖私有 cultivation semantic validator；外置会把内部恢复约束误变成公共构造 API。
  - `redis_hash_restore_fails_when_every_entry_is_invalid`：依赖私有批量 restore 的全失败聚合路径；外置会公开内部错误归并方式。
  - `bong_dormant_tick_interval_env_overrides_default`：直接调用私有环境配置解析；外置会把运行时配置解析实现当成稳定 API。
  - `bong_dormant_tick_interval_unset_keeps_default`：直接锁定私有 interval fallback；外置会固化内部默认值来源与解析层。
  - `bong_dormant_tick_interval_zero_and_garbage_fall_back_gracefully`：直接锁定私有配置容错分支；外置会公开内部解析与降级实现。
  - `bong_sim_seed_makes_combat_deterministic`：依赖私有 sim-seed 注入与 combat 装配；外置会把测试驱动的内部 seed 接口暴露为生产 API。
  - `bong_sim_seed_unset_or_garbage_defaults_to_zero`：直接锁定私有环境 seed parser；外置会固化配置解析细节。
  - `seed_rogue_faction_is_deterministic_per_char_id`：直接调用私有 rogue faction seed；外置会暴露 NPC 初始化算法。
  - `seed_rogue_faction_distribution_yields_both_attack_and_defend`：直接锁定私有 faction 分布表；外置会把内部采样表变成 API。
  - `seed_rogue_faction_pairs_are_hostile_across_factions`：依赖私有 faction seed 与 hostility 配对装配；外置会固化内部关系生成路径。
  - `dormant_rogue_seed_snapshot_assigns_non_none_faction`：依赖私有 rogue snapshot builder；外置会公开 NPC 快照构造细节。
  - `seed_emergent_group_is_deterministic_per_char_id`：直接调用私有 emergent-group seed；外置会暴露内部分组算法。
  - `seed_emergent_group_distribution_covers_at_least_three_groups`：直接锁定私有 group 分布表；外置会固化非生产 API 的采样细节。
  - `dormant_rogue_seed_snapshot_assigns_explicit_emergent_group`：依赖私有 snapshot 装配中的 group 注入；外置会把内部字段装配公开。
  - `realm_distribution_tables_sum_to_exactly_1000_per_mille`：直接读取私有 realm distribution tables；外置会把内部平衡表变成稳定 API。
  - `realm_distribution_tables_never_seed_void_naturally`：直接读取私有 realm 分布表的实现约束；外置会暴露 seed 平衡实现。
  - `sample_rogue_seed_realm_is_deterministic_per_char_id_and_zone_kind`：直接调用私有 realm sampler；外置会固化内部 salt/采样函数。
  - `sample_rogue_seed_realm_differs_by_zone_kind_salt_not_faction_or_group_salt`：验证私有 salt 选择算法；外置会把内部哈希布局误变成生产契约。
  - `sample_rogue_seed_realm_background_distribution_matches_table_within_tolerance`：直接依赖私有 background 分布表与 sampler；外置会公开内部平衡参数。
  - `sample_rogue_seed_realm_resource_distribution_matches_table_within_tolerance`：直接依赖私有 resource 分布表与 sampler；外置会固化内部平衡参数。
  - `dormant_rogue_seed_snapshot_realm_distribution_not_always_awaken`：依赖私有 seed snapshot 与 realm sampler 组合；外置会公开初始化实现。
  - `dormant_rogue_seed_snapshot_meridian_system_matches_sampled_realm_required_meridians`：依赖私有 realm-to-meridian 装配；外置会把初始化中间结构变成 API。
  - `dormant_rogue_seed_snapshot_qi_current_stays_zero_regardless_of_sampled_realm`：依赖私有 seed snapshot 初始化路径；外置会固化未 hydrate 快照的内部字段约定。
  - `dormant_rogue_seed_snapshot_same_seed_twice_produces_identical_realm`：依赖私有 sampler/snapshot 组合；外置会公开内部随机源接线。
  - `dormant_rogue_seed_snapshot_resource_vs_background_flag_changes_distribution`：直接锁定私有 resource/background 采样分支；外置会把内部分布算法当成 API。
  - `combat_terminal_events_wait_for_pending_hash_receipt`：依赖私有 combat terminal system 与 pending Redis receipt 装配；外置会公开异步结算内部时序。
  - `failed_pending_combat_hash_receipt_preserves_all_terminal_state_until_retry_succeeds`：依赖私有 pending combat retry state machine；外置会固化内部持久化重试结构。
  - `combat_terminal_settles_daozhan_cultivation_and_drain_owners_after_hash_receipt`：依赖私有 terminal receipt 后的 Daozhan/owner 结算编排；外置会暴露内部事务阶段。
  - `legacy_pending_combat_without_winner_still_settles_and_terminates`：依赖私有 legacy pending normalization 与终止路径；外置会把兼容实现细节变成 API。
  - `combat_death_releases_all_qi_to_zone`：依赖私有 combat settlement system 的事件、owner 与 zone 装配；外置会迫使 qi 结算内部入口公开。
  - `combat_death_emits_notice_with_combat_reason_and_pos`：依赖私有 combat phase event emission；外置会固化内部 notice 生成时序。
  - `combat_death_removes_loser_from_store`：依赖私有战斗结算对 store/index 的联动；外置会公开内部人口回写步骤。
  - `winner_qi_unchanged`：依赖私有 combat winner/loser settlement 装配；外置会把内部 owner 更新顺序变成 API。
  - `zone_full_settles_loser_into_fixed_overflow_without_shadow`：依赖私有 zone-full overflow settlement；外置会公开守恒实现的内部中转细节。
  - `settlement_failure_retains_loser_and_marks_store_dirty`：依赖私有 settlement failure/pending store 状态机；外置会固化失败恢复实现。
  - `sequential_release_no_overflow`：依赖私有连续 release orchestration；外置会把内部释放顺序误当公共接口。
  - `offscreen_war_conserves_physical_owner_total_without_actor_or_zone_shadows`：依赖私有完整 ECS combat harness 与 owner accounting fixture；外置会要求暴露运行时系统装配而非仅验证公开结果。
  - `combat_death_emits_pending_relic_for_named_disciple`：依赖私有 relic eligibility、pending event 与 combat settlement pipeline；外置会公开遗物内部生成阶段。
  - `no_relic_emitted_for_factionless_rogue_through_full_combat_tick`：依赖私有完整 combat tick 与 relic eligibility 装配；外置会把内部判定链条变成 API。
  - `settlement_failure_emits_no_relic_and_pending_loser_stays_frozen`：依赖私有失败结算与 pending relic 抑制状态机；外置会固化中间状态实现。
  - `pending_release_retries_then_emits_one_relic_after_zone_recovers`：依赖私有 pending-release retry pipeline；外置会公开重试/事件去重内部结构。
  - `pending_release_retry_freezes_per_char_state_and_preserves_owners_and_audit_each_tick`：依赖私有 per-character freeze、owner audit 与 tick 装配；外置会把内部审计步骤固化为 API。
  - `failed_pending_release_roundtrips_without_duplicate_events_then_finalizes_once`：依赖私有 pending-release Redis roundtrip/retry state machine；外置会暴露持久化内部表示。
  - `migration_marker_path_pinned_to_spec`：直接读取私有 migration marker path；外置会把部署内部路径常量变成生产 API。
  - `no_marker_triggers_reroll_and_writes_marker_file`：依赖私有 migration marker I/O 与 reroll orchestration；外置会公开启动迁移内部流程。
  - `migration_reroll_resyncs_meridian_system_to_new_realm_required_meridians`：依赖私有 migration reroll/meridian resync helper；外置会固化升级实现细节。
  - `migration_reroll_respects_permanently_severed_meridians`：依赖私有 migration reroll 与 severed-meridian 装配；外置会公开内部迁移步骤。
  - `marker_already_exists_skips_reroll_idempotently`：依赖私有 marker existence/idempotency flow；外置会把迁移控制面实现变成 API。
  - `identity_archetypes_write_identity_realm_not_sampled`：依赖私有 migration identity-archetype branch；外置会固化内部初始化分支。
  - `marker_write_failure_does_not_silently_swallow_error`：依赖私有 marker write error plumbing；外置会公开文件迁移适配层。
  - `empty_store_writes_marker_without_touching_anything`：依赖私有空 store migration system；外置会把启动迁移顺序固化为生产 API。
  - `migration_pushes_zone_perception_narration_for_upgraded_realms`：依赖私有 migration ECS/narration wiring；外置会暴露内部事件装配而不是稳定 narration 契约。
- **公开 API 与 seam**：A 类只使用既有 `bong_server::npc::dormant`、`qi_physics`、zone、serde 与 combat 公开行为；B 类使用同 crate 既有私有项。`ids_by_archetype` 与 `ids_by_zone` 仍保持原 `#[cfg(test)] pub fn`，没有放宽可见性、删除 `#[cfg(test)]` 或新增 `pub`/`pub(crate)`/`#[doc(hidden)]` seam，也没有复制生产实现。已在 `server`、`scripts`、`tests` 范围 grep `include_str!`、`production_source` 与 `npc/dormant/mod.rs`，确认没有测试消费者按源码文本读取该生产文件。
- **主线与门禁证据**：本批提交前完成 A/B 对拍和 `git diff --check`；后续提交前紧邻执行 `git fetch origin && git merge origin/main`，若 Cargo/plan 冲突保留双方条目并重跑受影响栈。最终绑定合并后 HEAD 的无上下文只读 validator 必须 PASS；完整 server fmt、clippy、test 三条均直接经 `scripts/build-token.sh` 取得真实退出码。PR body 将逐条保留以上 75 条 B 类理由。每个提交带 `Model: gpt-5.6-luna`。本条仅记录 P2-23 进度，P2 总体、P3、P4 仍未完成，plan 不归档。

- **合并主线后复验**：紧邻执行 `git fetch origin && git merge origin/main`，基于 `origin/main=187eeaf034de948a8c114a473f48129ad0261d88` 无冲突生成合并 HEAD `c45f2a6bb29752de379eafdfe1e5333409545cc0`；合并带入的 network/plan 变更未触及本批 dormant、Cargo target 或 P2-23 evidence。合并后直接经 `scripts/build-token.sh` 的完整 server gate 三条均为真实 `PIPESTATUS[0]=0`：fmt、clippy、test；library `11987 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`、所有 integration targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`。
- **合并后逐位对拍**：`cargo test --test npc_dormant_unit` 为 A 类 `23 passed / 0 failed / 0 ignored`，`cargo test npc::dormant::tests:: --lib` 为 B 类 `75 passed / 0 failed / 0 ignored`，两条命令均直接经 `scripts/build-token.sh`，分别取得 `A_PIPESTATUS[0]=0`、`B_PIPESTATUS[0]=0`；`23 + 75 = 98`，测试名集合、断言、边界与 fixture 语义未变。后续 validator 绑定本批最终 HEAD 后必须 PASS；P2 总体、P3 后续阶段、P4 仍未完成，plan 不归档。
- **最新主线冲突收口与复验**：基于 `origin/main=e52a991fdb26abe82e7fe66b68c31f42de9025ae` 紧邻执行 `git fetch origin && git merge origin/main`；仅 `docs/plan-test-layout-refactor-v1.md` 发生冲突，已保留 P2-18、P2-21、P2-22 等主线条目并将本批改为唯一空闲编号 P2-23。`server/Cargo.toml` 的全部既有 `[[test]]` targets（含 `tribulation_unit`、`proto_convert_unit`、`npc_dormant_unit`）均保留；合并 HEAD 为 `32bdaedec80ed09ec00af8f6d3d4adf5cd759134`。
- **最新 HEAD 门禁证据**：在 `32bdaedec` 上直接经 `scripts/build-token.sh` 重跑完整 server gate，fmt、clippy、test 均取得真实 `PIPESTATUS[0]=0`；library `11987 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`、全部 integration targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`。本次只解决 Cargo/plan 合流与编号，不改 `death_qi_release_repays_negative_zone_without_clamping_signed_state` 或任何测试断言、fixture、生产守恒路径；P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-22 network/redis_bridge（✅ 2026-09-08）

- **范围与落点**：仅将 `server/src/network/redis_bridge.rs` 原 `#[cfg(test)] mod redis_bridge_tests` 主测试体外置到同目录 `server/src/network/redis_bridge_tests.rs`，由生产文件 `:2955-2957` 的 `#[cfg(test)] #[path = "redis_bridge_tests.rs"] mod tests;` 挂载。任务卡统计的 83 个字面 `#[test]` 全部迁移，另有同一模块中的 9 个 `#[tokio::test]` 也一并迁移，因此没有任何主测试体留在生产文件；生产文件仍保留 :442/:458 两个既有 test-only helper，未改其行为或可见性。B 类为 92，A 类为 0；不新增 Cargo test target。
- **迁移对拍与行为**：基线卡片过滤器 `network::redis_bridge::tests:: --lib` 为 `0 passed / 0 failed`，原因是基线模块实际名为 `redis_bridge_tests` 而非 `tests`；使用真实基线过滤器 `network::redis_bridge::redis_bridge_tests:: --lib` 得 `92 passed / 0 failed / 0 ignored`。迁移后 `network::redis_bridge::tests:: --lib` 得 `92 passed / 0 failed / 0 ignored`，其中 `83` 个普通测试与 `9` 个异步测试逐项保留。测试名称、函数顺序、断言、fixture 文案、错误/边界语义及 5 处 `include_str!` 资源引用逐位对拍一致，未合并、删除或重排测试。
- **`include_str!` 路径与源码消费者**：5 处资源引用（`world-state.sample.json`、`chat-message.sample.json`、`spirit-treasure-dialogue-request.sample.json`、`agent-command.sample.json`、`spirit-treasure-dialogue.sample.json`）全部留在与生产源文件同目录的 B 类测试文件中，路径字符串原样保留，因此宏相对源文件解析的深度不变。已 grep `scripts/`、`server/tests/` 与 `.github/`，没有测试/脚本通过 `include_str!`、字面文本或其它源码读取方式消费 `redis_bridge.rs` 本身。
- **生产边界与 seam**：`server/src/network/redis_bridge.rs` 的生产实现只增加外置模块挂载，未改 Redis key、channel、payload、serde、schema、client、agent、玩法或 wire；未改 `server/Cargo.toml`、samples 或其它 plan。没有新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，也未复制生产实现。
- **提交与验证证据**：代码迁移提交为 `b12e6e736`（2026-09-08，带 `Model: gpt-5.6-luna`）；随后紧邻 `git fetch origin && git merge origin/main` 合入 `origin/main=187eeaf034de948a8c114a473f48129ad0261d88`，产生合并 HEAD `47d16ae538319cd8baa9cc2a919514df0cb074ea`，保留主线 P2-21 条目。无上下文只读 validator 分别对迁移代码 HEAD `b12e6e73653bbe4b8e4c01fb6d77d5d6075444e0` 与合并代码 HEAD `47d16ae538319cd8baa9cc2a919514df0cb074ea` 对拍并 PASS；后者确认生产逻辑、wire/schema/API seam、测试体、测试名断言及 `include_str!` 路径均未发生非迁移变化。合并后无外层 flock 的完整 server gate 均通过：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 均 exit 0；库 `12010 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`、全部 Cargo targets 通过，doc-tests `3 passed / 0 failed / 5 ignored`。合并后定向 `network::redis_bridge::tests:: --lib` 为 `92 passed / 0 failed / 0 ignored`、exit 0。每个提交带 `Model: gpt-5.6-luna`；P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-26 network/vfx_animation_trigger（✅ 2026-09-09）

- **范围与落点**：仅将 `server/src/network/vfx_animation_trigger.rs` 原 L1898 起主 `#[cfg(test)] mod tests` 的 97 条测试体原样外置到 `server/src/network/vfx_animation_trigger_tests.rs`，由生产文件末尾的 `#[cfg(test)] #[path = "vfx_animation_trigger_tests.rs"] mod tests;` 挂载；生产文件由 5,197 行降至 1,900 行。L216 的既有 `#[cfg(test)] pub(crate) const P1_WIRED_ANIM_IDS` 双端一致性测试常量表原样保留，不搬、不改可见性、不去掉 cfg 门。未新增 Cargo test target。
- **契约分类与 B 类边界**：97 条均为 B 类，依赖同 crate 私有 VFX system/helper、ECS 组件装配、私有状态/事件 fixture 或内部常量；integration crate 无法在不新增或扩大 test-only seam 的情况下复建完整 `bong:vfx_event` 事件副作用链，外置会把实现内部装配固化为生产 API。每条 B 类测试的具体私有依赖与“外置会扭曲生产 API”理由逐条写入 PR body。没有复制生产实现。
- **迁移对拍与 wire 边界**：迁移前 `cd server && ../scripts/build-token.sh cargo test network::vfx_animation_trigger::tests:: --lib` 为 `97 passed / 0 failed / 0 ignored`；迁移后同一命令为 `97 passed / 0 failed / 0 ignored`，名称集合差集为 0，原测试体在去除外层模块缩进并规范格式后内容逐位相同。测试名、断言、fixture、粒子/动画参数、`bong:vfx_event` ID 与边界语义均未改；生产 VFX/动画逻辑、schema、client、agent、Redis、qi 均未改。
- **两处 cfg(test) 核验与 seam**：仅 L216 的既有 `P1_WIRED_ANIM_IDS` 常量表和新挂载门保留在生产文件；零新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` seam。已确认该文件无 `include_str!` 源码文本消费者或相对路径陷阱。
- **提交与后续状态**：完整 server fmt、clippy、test 及最终主线复验按任务卡执行，validator 绑定最终 HEAD；本条仅记录 P2-26 进度，P2 总体、P3、P4 仍未完成，plan 保持 active、不归档。
### P2-24 schema client_request（✅ 2026-09-09）

- **范围与落点**：仅将 `server/src/schema/client_request.rs:970-3878` 原 `#[cfg(test)] mod tests` 的 154 个 `#[test]` 测试体外置到同目录 `server/src/schema/client_request_tests.rs:1-2899`；生产文件仅保留 `:970-972` 的 `#[cfg(test)] #[path = "client_request_tests.rs"] mod tests;` 挂载。未新增 Cargo test target，未改变生产 `ClientRequestV1`、serde `deny_unknown_fields`、C2S wire/schema、TypeBox sample、client、agent 或其它模块。
- **迁移前后对拍**：先用 `cd server && ../scripts/build-token.sh cargo test schema::client_request:: --lib -- --list` 验证真实选择器，得到 `154 tests, 0 benchmarks`；卡片中未经验证的过滤器未被照抄。迁移前、迁移后均用 `cd server && ../scripts/build-token.sh cargo test schema::client_request:: --lib`，分别为 `154 passed / 0 failed / 0 ignored` 与 `154 passed / 0 failed / 0 ignored`；154 个测试名及顺序、断言、边界/错误语义、fixture 经格式归一化逐项一致，异步属性口径一并计入。
- **`include_str!` 与源码消费者**：17 个既有 `include_str!` 文本引用（其中 12 个宏调用、5 个既有 pin 注释）均留在同 crate 同目录测试文件；宏参数/相对路径逐项一致，未修改字符串。已 grep `scripts/`、`server/tests/` 与 `.github/`：`scripts/check_c2s_gate_matrix.py:11,354` 仅读取固定生产路径并解析 `ClientRequestV1` enum 声明，`.github/workflows/c2s-gate-matrix.yml:8,21` 仅作路径触发；没有读取 inline 测试体或按测试 marker/挂载声明截断的消费者。
- **生产边界与 seam**：测试外置只依赖同 crate 既有父模块项，未新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam；生产前缀逐字一致，生产文件不再含测试属性/测试体，未复制实现。
- **完整 server gate**：不包外层 flock，直接执行 `../scripts/build-token.sh cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，三条真实 `PIPESTATUS[0]` 均为 `0`。完整测试库 `12010 passed / 0 failed / 1 ignored`，main `18 passed / 0 failed / 0 ignored`，全部 integration targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`。
- **最新主线合入后复验**：紧邻执行 `git fetch origin && git merge origin/main`，以 `origin/main=3748d52a7` 为合并输入；`server/src/schema/client_request.rs` 的生产容量校验采用主线 `HOTBAR_SLOT_COUNT`，外置测试同步主线快捷栏边界内容，P2-23、P2-24、P2-25、P2-27 条目并列保留。合并后对拍仍为生产原测试 154 → 外置 154（含异步属性口径），17 个 `include_str!` 路径字符串逐字一致；直接经 `scripts/build-token.sh` 的 fmt/clippy/test 真实退出码均为 `0`，library `11990 passed / 0 failed`、全部 integration targets 与 doc-tests 通过。因主线已将实体 hotbar 容量改为 2，合并树同步修正 `dying_elder_tests.rs` 的测试夹具以将超出 hotbar 的丹放入既有容器检索路径，不改变生产逻辑。
- **提交与验证证据**：代码迁移提交为 `9627b9d55bcd346283d5bc97d6f67da2548a56b0`（带 `Model: gpt-5.6-luna`）；紧邻执行 `git fetch origin && git merge origin/main`，基于 `origin/main=e52a991fdb26abe82e7fe66b68c31f42de9025ae` up-to-date。无上下文只读 validator 绑定完整 HEAD `9627b9d55bcd346283d5bc97d6f67da2548a56b0` 并 PASS（模型：`gpt-5.6-luna`）。P2 总体、P3、P4 仍未完成，plan 不归档。
### P2-25 npc ambient scheduler（⏳ 2026-09-09）

- **范围与落点**：仅处置 `server/src/npc/spawn/ambient_scheduler.rs` 原 `#[cfg(test)] mod tests`（基线源文件 5,550 行、测试模块从 L1378 起）的 114 条测试；全部为 B 类，原样外置到同 crate 同目录 `server/src/npc/spawn/ambient_scheduler_tests.rs`，生产文件只保留 `#[cfg(test)] #[path = "ambient_scheduler_tests.rs"] mod tests;` 挂载。114 条均依赖 `ambient_scheduler` 的模块私有函数、枚举、结构体或同 crate ECS/qi 结算装配（如 `round_to_stride`、`resolve_ambient_ground_position`、`submit_ambient_spawn_candidate`、`settle_rat_recycle`、`settle_spider_recycle` 与私有测试所需的内部路径），外置为 integration test 会迫使非稳定实现细节进入生产 API，故不新增 A 类目标、不新增 Cargo `[[test]]` target。生产段无其它 `#[cfg(test)]` helper/形参项，本批无项可保留。
- **迁移对拍与筛选**：先执行完整列表 `cd server && ../scripts/build-token.sh cargo test --lib -- --list`，真实过滤器为 `npc::spawn::ambient_scheduler::tests::`，命中 114；迁移前 `cd server && ../scripts/build-token.sh cargo test --lib 'npc::spawn::ambient_scheduler::tests::'` 为 `114 passed / 0 failed / 0 ignored`，迁移后同过滤器仍为 `114 passed / 0 failed / 0 ignored`，包含全部测试属性（本批扫描到 114 个 `#[test]`，无异步测试属性遗漏）。测试名、函数顺序、断言、fixture、错误/边界行为逐位保持；初始错误过滤器未匹配到列表输出时未计入基线。
- **源码锚点与 seam**：已复核 `ambient_scheduler.rs` 与 `server/src`、`server/tests`、`scripts`、`.github`，无 `include_str!` 读取该源码，也无其它源码文本消费者；没有新增/扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，未改生产逻辑、spawn 规则、调度节奏、事件、qi_physics、Cargo 或跨栈文件。对应代码提交 `930950617e7ffc3bf4a3cd7a4ba51c574280cf92`、格式修正提交 `e515b0758760580be89574fe2c18f3770b6bdbf8`，均带 `Model: gpt-5.6-luna`。代码/服务器输入 HEAD `6e5170d9be2282aad14a59b8b274e8e39b3a7b09` 已通过无上下文只读 validator（`PASS @ 6e5170d9be2282aad14a59b8b274e8e39b3a7b09`，模型 `gpt-5.6-luna`）：三文件范围、生产挂载、114 过滤命中、无 Cargo `[[test]]` target、无 seam 与源码消费者均核验通过。随后紧邻执行 `git fetch origin && git merge origin/main`，`origin/main=bb4b21dc8bf3c97c2afaad3d4b7bce01aa0fd353` 已是最新，代码输入 HEAD 未变；基于该代码输入 HEAD 执行无外层 flock 的完整 server gate——`cd server && ../scripts/build-token.sh cargo fmt --check`、`clippy --all-targets -- -D warnings`、`cargo test`——三条均 exit `0`，library `11988` 个测试及全部 integration targets、doc-tests 全部通过。之后的提交 `60fcaad10c404b28c21949afb6be74072068cbb5` 与 `999968af2ff368709d2cc2a24d3637fc1b9c1c2c` 仅更新本计划 evidence，不改变 server 输入；最终分支输入 HEAD `999968af2ff368709d2cc2a24d3637fc1b9c1c2c` 已重新通过无上下文只读 validator（模型 `gpt-5.6-luna`）及无外层 flock 的完整 server gate（fmt/clippy/test 均 exit `0`，library `11988` 个测试及全部 integration targets、doc-tests 全部通过）。本条仅记录 P2-25，P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-27 fauna dying_elder（⏳ 2026-09-09）

- **范围与落点**：仅处置 `server/src/fauna/dying_elder.rs` 原 `#[cfg(test)] mod tests`（基线源文件 5,192 行、测试模块从 L1761 起）的 85 条测试；全部为 B 类，原样外置到同 crate 同目录 `server/src/fauna/dying_elder_tests.rs`，生产文件只保留 `#[cfg(test)] #[path = "dying_elder_tests.rs"] mod tests;` 挂载。不新增 `server/Cargo.toml` 的 `[[test]]` target。
- **迁移对拍与筛选**：先执行 `cd server && ../scripts/build-token.sh cargo test --lib fauna::dying_elder::tests:: -- --list`，真实过滤器命中 `85 tests / 0 benchmarks`；迁移后 `cd server && ../scripts/build-token.sh cargo test --lib 'fauna::dying_elder::tests::'` 为 `85 passed / 0 failed / 0 ignored`。85 条普通 `#[test]` 均保留，测试名、函数顺序、断言、fixture、错误/边界行为逐位保持；未发现异步测试属性遗漏。
- **源码消费者与生产边界**：已复核 `server/src`、`server/tests`、`scripts` 与 `docs`，没有 `include_str!` 读取 `dying_elder.rs`，也没有其它源码文本消费者；未改生产逻辑、事件、掉落/交互契约、qi_physics、schema、client、agent 或其它 plan。生产文件无测试体残留，仅保留挂载；无新增/扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam。
- **提交与验证证据**：迁移提交 `b5b985569`、Rustfmt 收口提交 `cce4bf0e9`（均带 `Model: gpt-5.6-luna`）；在代码输入 HEAD `cce4bf0e91824b895508ff05983b63a304b3f163` 上，无上下文只读 validator PASS。随后执行 `git fetch origin && git merge origin/main`，当前 `origin/main=154705a3251eb3ebb396f29fc27787a905405524` 已是最新，Already up to date，无冲突。该代码输入 HEAD 的完整 server gate（直接经 `scripts/build-token.sh`，无外层 flock）三条均 exit 0：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`；library `11987 passed / 0 failed / 1 ignored`，main `18 passed / 0 failed / 0 ignored`，全部 integration targets 通过，doc-tests `3 passed / 0 failed / 5 ignored`。本条仅记录 P2-27，P2 其它模块、P3、P4 仍未完成，plan 不归档。

### P2-28 fauna daozhan（⏳ 2026-09-09）

- **范围与落点**：仅将 `server/src/fauna/daozhan.rs` 原 `#[cfg(test)] mod tests` 的全部 79 个测试外置到同 crate 同目录 `server/src/fauna/daozhan_tests.rs`；生产文件现仅保留 `#[cfg(test)] #[path = "daozhan_tests.rs"] mod tests;` 挂载。全部测试均为 B 类，未新增 `server/Cargo.toml` 的 `[[test]]` target。
- **迁移前后对拍与筛选**：迁移前先执行 `cd server && ../scripts/build-token.sh cargo test --lib -- --list`，从完整列表确认真实选择器为 `fauna::daozhan::tests::` 且命中 79 条；迁移前、迁移后均执行 `cd server && ../scripts/build-token.sh cargo test --lib 'fauna::daozhan::tests::'`，分别为 `79 passed / 0 failed / 0 ignored`。测试名序列差集为 0，原测试函数、断言、fixture、错误/边界语义与执行顺序保持不变；本批仅有普通 `#[test]`，无遗漏异步测试属性。
- **B 类理由与 seam**：79 条测试均直接使用 `daozhan` 模块私有类型、函数、常量或同 crate Bevy/ECS 装配；外置到 integration crate 将迫使这些实现细节成为生产 API，故统一保留同 crate 路径。已核对该源码无 `include_str!` 或其它源码文本消费者；未新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam，未复制生产实现。
- **生产边界**：未改道伥状态、伪装/伏击、掉落、天道凝结、死亡真元释放、事件、system 注册顺序、schema、wire、Redis、client、agent、qi_physics 或其它测试迁移范围；生产文件从 3,101 行收缩至测试挂载与生产实现共 1,335 行，测试体完整落在独立同 crate 文件。
- **提交与后续状态**：代码迁移对应 `ce96346d9`（2026-09-09，带 `Model: gpt-5.6-luna`）；本条仅记录 P2-28 进度，P2 总体、P3、P4 仍未完成，plan 保持 active、不归档。
### P2-29 alchemy/pill（⏳ 2026-09-09）

- **范围与落点**：仅处置 `server/src/alchemy/pill.rs` 原唯一 `#[cfg(test)] mod tests` 的 89 条测试；全部作为 B 类原样外置到同 crate 同目录 `server/src/alchemy/pill_tests.rs`，生产文件仅保留 `#[cfg(test)] #[path = "pill_tests.rs"] mod tests;` 挂载。不新增 `server/Cargo.toml` 的 `[[test]]` target，A 类为 0。
- **`include_str!` 与源码消费者**：10 处既有 `include_str!` 全部留在同 crate 测试文件，宏参数逐字保留，因此相对生产源文件的路径深度不变；已复核 `server/src`、`server/tests`、`scripts`、`docs`，没有其它文件按 `pill.rs` 源码文本或挂载声明读取它。
- **契约分类与边界**：89 条均依赖 `alchemy::pill` 同 crate 私有丹药规格、消费/毒性/伤口处理与内部 fixture 装配；外置为 integration test 会把实现私有项固化为生产 API，故全部留 B 类。测试名、断言、边界、错误语义和 fixture 不改；不碰丹药生产逻辑、事件/wire、schema、client、agent、`qi_physics` 或任何守恒路径。
- **迁移对拍与 seam**：迁移前真实 `alchemy::pill::tests::` 列表命中 `89 tests`，迁移后定向运行 `89 passed / 0 failed / 0 ignored`，异步属性口径一并计入；生产前缀逐字一致，零新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` seam。本条仅记录 P2-29 进度，P2 总体、P3、P4 仍未完成，plan 不归档。

### P2-30 zhenfa（✅ 2026-09-12）

- **范围与落点**：仅处置 `server/src/zhenfa/mod.rs` 原 9,023 行内联测试；生产文件删除测试体并保留 `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;` 挂载。53 条 A 类外置到 `server/tests/unit/zhenfa/zhenfa_test.rs`，并由 `server/Cargo.toml` 的 `zhenfa_unit` target 发现；34 条 B 类保留在 `server/src/zhenfa/mod_tests.rs`，未复制生产实现、未改变 zhenfa 运行时、事件、物品、区块写入或真元路径。
- **A/B 分类与逐位守恒**：A 类 53 条验证公开 API 驱动的 ECS/IO、副作用、物品消费、区块写入、事件/VFX/narration、音频 recipe 等外部可观察契约；B 类 34 条依赖 `zhenfa` 同 crate 私有纯逻辑、私有 fixture 装配或内部 system 链，外置会迫使实现细节进入生产 API，具体逐条理由列于 PR body。迁移前用 `#[(test|tokio::test)]` 口径核得 87 条，迁移后 `cargo test --lib zhenfa::tests::` 为 34 passed、`cargo test --test zhenfa_unit` 为 53 passed，守恒式为 `53 + 34 = 87`；测试名、断言、错误/边界语义保持不变。
- **fixture 与 seam**：外置测试中的音频 fixture 统一使用 `../../../assets/audio/recipes/...`，相对 `server/tests/unit/zhenfa/` 可正确解析；除新增 Cargo test target 与模块挂载外，没有新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam。既有生产 API、schema、wire、Redis、client、agent、`qi_physics` 与跨模块行为均未改动。
- **验证与门禁**：无上下文只读 validator 绑定最终 HEAD `4d904e16ff8500f113a9a4eae65a4b61a2cea977` 并 PASS。随后紧邻执行 `git fetch origin && git merge origin/main`，输入 `origin/main=9ea621a400c34c9807cd11afdf2cbd6f6dbe24de`，结果 `Already up to date`；因此无需合并后代码复验。server 完整 gate 的 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 真实退出码均为 `0`；全量结果为 library `11929 passed / 0 failed / 1 ignored`、main `18 passed / 0 failed / 0 ignored`，所有 integration targets 及 doc-tests 无失败（doc-tests `3 passed / 0 failed / 5 ignored`），其中 `zhenfa_unit` `53 passed / 0 failed / 0 ignored`、同 crate zhenfa `34 passed / 0 failed / 0 ignored`。
- **提交与状态**：迁移及收口提交依次为 `ea5782c00`、`a43010d76`、`3d781559f`、`23ff15d9c`、`4d904e16f`，均带 `Model: gpt-5.6-luna`。本条仅记录 P2-30 进度；P2 总体、P3、P4 仍未完成，plan 保持 active、不归档。

### P2-31 player/state（✅ 2026-09-13）

- **范围与落点**：仅处置 `server/src/player/state.rs` 原有 78 个 `#[test]`/`#[tokio::test]`；生产文件删除内联测试体并保留 `#[cfg(test)] #[path = "state_tests.rs"] mod tests;` 挂载。30 条 A 类外置到 `server/tests/unit/player/state_test.rs`，由 `server/Cargo.toml` 的 `player_state_unit` target 发现；48 条 B 类保留在 `server/src/player/state_tests.rs`。未改 PlayerState 生产逻辑、SQLite slice/迁移、库存、生命周期、功法、棺材、真元或跨模块运行时行为。
- **A/B 分类与逐位守恒**：A 类 30 条验证既有公开 PlayerState/持久化 API 的 SQLite/库存/生命周期/棺材副作用与错误结果；B 类 48 条依赖 `state.rs` 同 crate 私有迁移/装配/边界 helper、私有 persistence 接口或 `#[cfg(test)]` JSON serializer 分支，外置会迫使实现细节成为生产 API，逐条理由列于 PR body。迁移前用 `#[(test|tokio::test)]` 全集口径核得 78 条；迁移后 `cargo test --test player_state_unit` 为 30 passed、`cargo test --lib player::state::tests::` 为 48 passed，逐位守恒 `30 + 48 = 78`；测试名、断言、fixture、错误/边界语义保持不变。
- **测试可见性与路径**：没有为 integration 测试新增或扩大 `pub`、`pub(crate)`、`#[doc(hidden)]` 或其它 test-only seam；`serializes_player_state_payload` 留在同 crate，是因为 `serialize_server_data_payload` 的 JSON 分支仅在 `#[cfg(test)]` 生效，生产 integration crate 走 protobuf，不能以改生产行为换取迁移。该模块无 `include_str!` 路径需要改写；未复制生产实现。
- **validator 与主线证据**：无上下文只读 validator 绑定代码 HEAD `114fe208d5b61f9939b48ca919f0b22b322d91ef` 并 PASS，确认 HEAD 对拍、工作区干净、测试计数 `48 + 30 = 78`、结构 diff 仅测试外置/target 挂载且无新增 seam。随后紧邻执行 `git fetch origin && git merge origin/main`，`origin/main=375b13cc783196d6e63506045346f244135d0aee`，结果 `Already up to date`，无合并冲突或新增变更。
- **完整门禁与提交**：`scripts/build-token.sh cargo fmt --check`、`scripts/build-token.sh cargo clippy --all-targets -- -D warnings`、`scripts/build-token.sh cargo test` 真实退出码均为 `0`；完整 cargo test 的 library 为 `11907 passed / 0 failed / 1 ignored`，main 为 `18 passed / 0 failed / 0 ignored`，新增 `player_state_unit` 为 `30 passed / 0 failed / 0 ignored`，全部既有 integration targets 与 doc-tests 无失败（doc-tests `3 passed / 0 failed / 5 ignored`）。代码迁移提交为 `114fe208d5b61f9939b48ca919f0b22b322d91ef`，带 `Model: gpt-5.6-luna`；本条仅记录 P2-31 进度，P2 总体、P3、P4 仍未完成，plan 保持 active、不归档。

### P2 测试准入策略重基线（✅ 2026-09-05）

- **保留的硬断言**：安全/权限、原子性/并发、真元守恒、真实状态机分支、跨进程或跨版本协议/schema、持久化兼容，以及已发生 bug 的最小回归。硬编码值仅在其本身是外部或领域契约时保留，例如 MC packet ID/编码顺序、文件权限、`qi_physics` 常量引用或明确版本化 payload tag；能引用生产常量时不得复制魔法数。
- **合并或替换**：多个 enum 值、输入值或 fixture 仅经过同一无分支路径时，保留能区分行为等价类的代表 case，必要时使用表驱动测试；不再因“每个 enum 变体”或“所有组合”本身而新增同构断言。
- **删除或改写**：字段总数、私有字段顺序、内部默认构造细节、放置算法的任意扫描顺序、sample 的叙事文案/演示 tick/地图名，以及仅 grep 源码函数名、变量名或 shell 写法的断言，除非它们已被证明是对外兼容契约。此类测试应删除，或改为验证不重叠、可解析、权限拒绝、失败清理等可观察结果。
- **既有 seam 审计**：`server/src/world/pseudo_vein_runtime.rs` 的 `set_test_state` 与 `from_test_snapshot` 是 P2-01 新增的明确测试 seam；`pseudo_vein_phase_narration`、`should_emit_visual`、`pseudo_vein_vfx_request` 与 `round3` 同时被运行时系统调用，但为外置测试而导出并标记为 `#[doc(hidden)]`。P4 对该组和其它 `#[doc(hidden)] pub` 项逐项记录生产消费者、受保护契约与处置；不能证明生产必要性的入口不得因测试布局而长期保留。
- **P4 前置**：在继续任何 inline 搬迁前，`inline-test-inventory.tsv` 必须为每个模块或同构测试组记录受保护契约、风险、处置、目标位置、是否需要私有访问及 seam 处置。四个首批复审模块用于验证该分类规则；测试数量变化必须可由该清单解释，但不需要与迁移前相同。

## P3 — CI 兼容、报告收口与迁移对拍（⏳）

- 先在 `.github/workflows/e2e.yml` 的 `server-test` job 进行 shadow run：`test-all.sh --profile unit --suite server` 与原 `cargo test` 并行执行，保留原命令、DAG、`evidence-server-test` artifact 和超时。
- 对拍至少覆盖退出码、要求 suite、受保护契约对应的测试选择、原生 Cargo 输出和 `.sisyphus/evidence/**`；测试计数只作诊断信息，变化须有分类理由但不自动阻塞切换。
- Client/Schema/Tiandao 仅接入其已有 canonical path 和原生命令，不借 P3 改源测试目录；统一报告只索引 Gradle/JUnit、Cargo、Vitest 和脚本原生产物，不强制新增无消费者的 JUnit 转换。

### P3-01 server-test shadow（✅ 2026-09-04）

- **范围与落点**：仅在 `.github/workflows/e2e.yml` 的 `server-test` job 增加 `../scripts/test-all.sh --profile unit --suite server` shadow；原有 `working-directory: server` 的 `../scripts/build-token.sh cargo test` 仍执行，未改 `needs`、25 分钟 timeout、`CARGO_TARGET_DIR`、`schema-dist` 下载、`evidence-server-test` 上传或下游 DAG/cleanup 语义。
- **run-private 对拍证据**：每次 CI 运行写入 `.sisyphus/evidence/test-all/server-shadow/<run-id>/`，shadow 保留 `summary.json`/`summary.tsv`、suite 原生日志；native 保留完整 `cargo test` 输出与命令/状态/退出码；`comparison.json`/`comparison.tsv` 记录 wrapper process/suite exit code、native exit code、按 Cargo target 汇总的 passed/failed/ignored/measured/filtered-out 计数、失败 suite/测试以及 `native-report.diff`。
- **失败传播**：shadow 与 native 均不后台执行；native 输出通过 `PIPESTATUS[0]` 取真实命令退出码，tee/报告错误分别失败；比较步骤不吞失败，native 非零优先按其真实码退出，native 成功而 wrapper 失败时传播 wrapper 码，双方成功但报告不一致时返回比较失败码。
- **验证证据**：修复 workflow 内嵌 Python 缩进后，`bash -n scripts/test-all.sh`、server-test run block 的 `bash -n`、两段内嵌 Python 的 AST 解析和 workflow/job/DAG 静态核验均通过；build-token contract `23/23`、`scripts/tests/test_all_contract_test.sh` `90/90`、CI entrypoint `11` 文件及 signal/fallback/proto/preview-lifecycle/cargo-target scope contracts 全部 PASS。shadow 与 native 在同一正确 server 工作目录串行对拍均为 `comparison=PASS`：wrapper process exit `0`、shadow suite exit `0`、native cargo exit `0`，两侧均为 `12877 passed / 0 failed / 7 ignored`，失败 suite/测试为空，规范化原生报告差异为 `0` 行；native 通过 `PIPESTATUS[0]` 记录真实退出码，无后台进程、`tail` 或失败吞没。
- **主线合并后 gate**：基于 `origin/main=fdabe0f0b` 执行 `git fetch origin && git merge origin/main`，无冲突，merge commit 为 `472bcd9b8`；合并后 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test shader_state_unit'` 为 `7 passed / 0 failed / 0 ignored`。随后指定 server gate exit `0`：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；library 为 `12641 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed / 0 ignored`，`shader_state_unit` 为 `7 passed / 0 failed / 0 ignored`，其余 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。
- **提交与审查证据**：P3 代码与严格报告解析对应 `2a73e9627`、`cb02d5fa0`、`5ef2b4cd1`；本 P3 evidence 对应本次中文 docs commit。原生 `cargo test` 命令仍保留，未切换、删除或替换；P3-01 已完成，P3 后续步骤仍未完成。
- **首次 CI 失败归因与修复（2026-09-04）**：PR #2167 首轮 `server-test` 暴露 `test-all.sh` 的 `unit:server` 额外执行 fmt/clippy，CI stable clippy 在既有 `npc/spawn`、`npc/territory`、`world/environment`、`network/client_request_handler`、`network/redis_bridge`、`npc/dormant/relic_hydrate` 代码上报 lint，导致 shadow wrapper exit `1`/suite exit `101`；同轮原生保留的 `cargo test` exit `0`、`12877 passed / 0 failed / 7 ignored`。修复后 unit server runner 与 P1 CLI 契约一致，仅执行 `build-token cargo test`，full server 仍保留 fmt/clippy/test/build；新增 contract pin，`test_all_contract_test.sh` 为 `95 passed / 0 failed`。
- **修复后本地最终对拍**：锁内串行执行 shadow 与同工作目录 native `cargo test`，两侧均 `exit 0`、30 个 target，`12877 passed / 0 failed / 7 ignored`，passed/failed/ignored 计数完全一致；workflow 静态核验与 `bash -n` 均通过。该修复仅触及 `scripts/test-all.sh` 及其 P3 对拍 contract，不改变 server 生产/玩法/schema。
- **最终主线合并后复验（2026-09-04）**：按收口要求紧邻执行 `git fetch origin && git merge origin/main`，基于 `origin/main=1cfec210df91357eb16329ca8ce6ae71a651ebdf` 无冲突生成 merge commit `a0238e0eeafc93ae3ecf5ffba52269b60e279ac7`；最终 HEAD 上 `shader_state_unit` 定向测试为 `7 passed / 0 failed / 0 ignored`，完整 server gate exit `0`，library 为 `12641 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed / 0 ignored`，所有 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。最终工作树仅保留本 P3 的 workflow 与本 plan evidence 变更。
- **最新主线合并后的受影响范围复验（2026-09-04）**：基于 `origin/main=c25f22762` 紧邻执行 `git fetch origin && git merge origin/main`，无冲突生成 merge commit `204f3d639`；`recipe_fragment_unit` 定向测试为 `3 passed / 0 failed / 0 ignored`，最终完整 server gate exit `0`，library 为 `12638 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed / 0 ignored`，`shader_state_unit` 为 `7 passed / 0 failed / 0 ignored`，全部 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。合并后的差异仍仅为 `.github/workflows/e2e.yml`、`scripts/test-all.sh`、`scripts/tests/test_all_contract_test.sh` 与本 plan evidence；bash 语法、95 项脚本 contract、内嵌 Python AST 及 workflow 静态核验均通过。
- **P2-15 主线合并后的最终 gate（2026-09-04）**：最新 `origin/main=dd7c63107` 包含主线 P2-15 forge history 外置变更；提交 evidence 后再次执行紧邻 `git fetch origin && git merge origin/main`，无冲突生成 merge commit `ade5a9ab9`。合并后 `recipe_fragment_unit` 为 `3 passed / 0 failed / 0 ignored`，`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 与完整 `cargo test` 均 exit `0`；library 为 `12636 passed / 0 failed / 2 ignored`，main binary 为 `18 passed / 0 failed / 0 ignored`，`shader_state_unit` 为 `7 passed / 0 failed / 0 ignored`，所有 integration targets 无失败，doc-tests 为 `3 passed / 0 failed / 5 ignored`。本轮 bash/脚本 contract、workflow 静态核验与 shadow/native 退出码及 passed/failed/ignored 对拍均保持 PASS。

## P4 — 剩余 Rust tests 的筛选、迁移与收口（⬜）

- 根据 P0 grep 快照维护 `inline-test-inventory.tsv`：模块/测试组、受保护契约、风险、处置（保留/合并/替换/删除）、目标路径、私有访问理由、现有或待删 seam、责任人、迁移 PR、对拍命令、状态。测试数量只作审计辅助，不是完成度指标。
- 以一个模块一个 PR 的节奏处理剩余 `server/src/**` 测试；每个 PR 必须先完成该模块分类，可外置必要行为测试、合并同构测试、删除实现镜像，或保留经说明的独立 `tests.rs`。PR 不混入运行时业务行为变化、无关格式化或跨模块 rename。
- 收口门禁：所有 P0 快照中的测试体和 P2 的测试专用 public seam 都有处置记录；生产实现文件不新增测试体；仅剩经登记、确有私有契约理由的独立 `tests.rs`；`scripts/test-all.sh --profile unit --suite server`、原生 Cargo 门禁和 server CI 全绿。
- P4 完成后才能将本 plan 全阶段标记 ✅。完成标准是必要契约的可验证覆盖和无测试专用 API 债务，不是 `#[cfg(test)]` 或测试数量机械归零。

### P4-前置 inline-test-inventory（✅ 2026-09-05）

- **清单落点与范围**：新增 `docs/inline-test-inventory.tsv`，以 P0 快照 `dd7c63107` 的 700 个模块行作为历史覆盖，并在当前 `origin/main=33053a55a7b6b79c9ee709ab07830e60d4dde6e7` 复核路径；`schema/client_payload.rs` 保留为已由 P2-16 外置的历史处置记录，当前额外发现的 `npc/scattered_cultivator.rs` 作为单独增量行登记。清单共 16 列，测试数量仅作诊断，不作为守恒验收。
- **首批逐条复审**：清单逐条登记 `schema/proto_gen.rs` 的 382 项、`network/client_request_handler.rs` 当前 270 项（快照诊断值 271）、`persistence/mod.rs` 的 172 项；另登记 `world/pseudo_vein_runtime.rs` 的既有 seam 审计。协议/兼容/错误、权限/原子性、持久化兼容、真元守恒与真实状态转换标为保留；同构 roundtrip 标为可合并；内部实现镜像/源码扫描不作为契约。
- **persistence 结论**：172 个 inline 测试约依赖 86 个私有生产符号，多数同时服务生产路径，未批量开放。首批 8 个 migration/bootstrap 测试只需 `apply_migrations`、`CURRENT_USER_VERSION`、`CURRENT_SCHEMA_VERSION` 与已有公开 `bootstrap_sqlite`；前三个旧 seam 均记录为“无独立生产消费者 → 待撤回”，临时目录和 database path 仅为测试 fixture；其余 migration/backup/runtime/social/qi 测试逐条记录为登记的 `server/src/persistence/tests.rs` 私有契约例外。
- **seam 审计**：全仓逐项记录 18 个 `#[doc(hidden)] pub` 声明，其中 11 个有生产消费者而保留，7 个仅测试消费者并标记待撤回/替换；`server_readiness::publish`、伪脉生命周期/VFX/舍入 helper 与 `tsy_container_search` 的四个安全/库存 helper 均以生产引用为据，不批量删除。未新增 public 或 `#[doc(hidden)]` seam。
- **对拍与边界**：每个首批测试记录 locked `cargo test` 过滤器与目标路径对拍命令；本批次仅分类/审计和 plan evidence，不搬迁、删除测试，不修改生产代码、Cargo、CI 或其它 plan。后续迁移必须以本清单为前置，并按当前 Kody-only 调度约束进行审查。

### P4 client_request_handler seam 回收（✅ 2026-09-07）

- **范围与落点**：按 `docs/inline-test-inventory.tsv` 已登记的 270 条 `server/src/network/client_request_handler.rs` 测试逐条复核；删除原文件 inline test body，保留 72 条父模块私有 helper/私有 ECS 契约，并将其余 198 条公开 ingress 契约整体回搬到登记的 `server/src/network/client_request_handler_tests.rs` 子模块。删除旧的 `client_request_handler_unit` integration target；`server/Cargo.toml` 其它 test/bench targets 保持不变。
- **契约与边界**：测试名称集合仍为 270；安全/权限、跨进程 dispatch、状态转换、库存/真元副作用与失败原子性断言均保留，未复制生产实现。198 条测试整体回搬的原因是它们注册 `handle_client_request_payloads` / `validate_and_dispatch_lingtian_requests` 及 `.after/.before/.chain/.in_set`，并直接构造私有 `PendingLingtianRequests`，integration target 无法在不开放 opaque `SystemParam` 的情况下编译。`PendingLingtianRequests`、`LingtianDispatchWriters`、`NpcEngagementRequestParams` 均收回为 `pub(crate)`；字段可见性未放宽，未新增 `pub`/`#[doc(hidden)]` seam，也未改变运行时逻辑、schema、wire、Redis、qi 或其它 gameplay 模块。
- **必要 fixture 收口**：`c2s_ingress_budget_drops_the_thirty_third_payload_before_decode` 只验证 ingress budget，改用既有无 lingtian handler fixture，避免 unit `cfg(test)` 下闲置 lingtian HUD snapshot 污染 wire 类型断言；原测试名、断言和边界保持不变。
- **文件行数核对**：`client_request_handler.rs` 实际为 7,653 行。删除 inline tests 后仍有被 `handle_client_request_payloads` 及 typed domain route 调用的既有生产 helper；本批次硬边界禁止移动/删除生产逻辑，因此不宣称已达到任务卡约 2,500 行 / master §8 `<3000`，后续若要继续收缩须另立生产重构范围。
- **逐条对拍**：登记模块私有测试 72、回搬 ingress 测试 198，集合并集 270；与 TSV 的 270 个 `network/client_request_handler.rs::...` 逐条记录相比无缺失、无新增。所有记录的目标路径统一为登记模块，三处测试专用 public seam 的回收已同步登记。
- **定向对拍与 validator**：当前 HEAD 已执行 `flock /tmp/bong-cargo.lock -c "cd server && ../scripts/build-token.sh cargo test network::client_request_handler::tests --lib"`，270 passed / 0 failed；临时恢复旧 integration target 的原 198 条对拍亦通过。无上下文只读 validator 绑定 `758a10cf7` 并 PASS，确认 270 个测试名与 591 个断言完整、无新增 seam。
- **提交与门禁证据**：代码/Cargo/测试落点收口提交为 `758a10cf7`（2026-09-07，含 `Model: gpt-5.6-luna`）；计划/TSV seam 更正提交为 `76c75e482`（含 `Model: gpt-5.6-luna`）。两次无上下文只读 validator 分别绑定 `758a10cf7`、`76c75e482` 并 PASS。基于当前 `origin/main=94119a7a7` 已执行紧邻 `git fetch origin && git merge origin/main`，无冲突；锁内完整 server gate（`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`）exit `0`：library `12174 passed / 0 failed / 1 ignored`，main binary `18 passed / 0 failed / 0 ignored`，全部已登记 integration targets（含 `recipe_fragment_unit=3 passed`）无失败，doc-tests `3 passed / 0 failed / 5 ignored`。保留所有既有 P2/P3/P4 条目与其它 Cargo targets；push、PR、CI/e2e/Kody 与 inline review 证据在 PR 开出后补录。
### P4 persistence/mod.rs 私有契约测试迁移（✅ 2026-09-05）

- **范围与落点**：按 `docs/inline-test-inventory.tsv` 已登记的 172 条逐条复核结果，将 `server/src/persistence/mod.rs` 的单一 `persistence_tests` 测试体原样移入 `server/src/persistence/tests.rs`，由 `#[cfg(test)] mod tests;` 作为 persistence 子模块编译；保留测试名称、fixture、断言、迁移/事务/WAL/并发/NPC/zone/player/social/qi durable 契约，不删除受保护测试，也不新增独立 integration target。
- **生产边界**：`mod.rs` 从实际 20,819 行降至 9,755 行，`tests.rs` 实际 10,996 行并保留 172 个 `#[test]`；生产实现、迁移链、SQL、表结构、事务边界和 R3 P0 接入点未改动。三条 inventory seam 记录经生产引用核验：`apply_migrations`、`CURRENT_USER_VERSION`、`CURRENT_SCHEMA_VERSION` 仍被生产 bootstrap/持久化路径使用，因此撤回的是测试专用访问路径，生产私有符号按硬边界保留。
- **私有契约理由**：约 86 个私有生产符号多数同时服务生产路径；同级 `tests.rs` 子模块可直接验证这些登记的私有契约，不需要新增 `pub`、`pub(crate)` 或 `#[doc(hidden)]` seam，不复制生产逻辑。公开可观察行为不足以覆盖迁移/schema/原子性/失败回滚边界的测试继续留在登记的 `server/src/persistence/tests.rs`。
- **验证**：迁移后定向命令 `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test persistence::tests --lib'` 通过，目标 persistence 测试 172 条全部通过（同过滤器同时命中既有 `mineral::persistence::tests` 17 条与 `spiritwood::persistence::tests` 19 条，合计 `208 passed / 0 failed / 0 ignored`）。完整 server gate `flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'` exit `0`：library `12626 passed / 0 failed / 2 ignored`，main binary `18 passed / 0 failed / 0 ignored`，全部 integration targets 无失败，doc-tests `3 passed / 0 failed / 5 ignored`。迁移前后测试名称集合 `172/172` 对拍一致；本 evidence 对应当前迁移提交，主线合并后将按流程复验受影响栈。

### P4 combat/resolve.rs 测试分类与迁移（⏳ 2026-09-06 复审中）

- **逐条分类结论**：`docs/inline-test-inventory.tsv` 的 129 条记录已逐条复核，不再把“调用公开 resolver”误写成“必须同级私有”。其中 18 条可由公开 `combat::resolve::resolve_attack_intents`、公开 ECS 组件/事件驱动，外置并合并为 `resolve_public_lifecycle_and_game_mode_matrix`、`resolve_public_attacker_lifecycle_matrix`、`resolve_public_game_mode_switch_is_live`、`resolve_public_reach_boundary_matrix` 及 6 个公开 API 单测；17 条同构私有纯决策 case 合并为 body-multiplier、prepaid-source、part-consequence 三个 matrix runner；1 条实现镜像删除。其余 93 条保留同级 `resolve_tests.rs`，每行在 TSV 记录了具体 helper/fixture 或私有纯函数依据，而不是整块默认保留。
- **私有边界依据**：真正依赖私有实现的是 110 条（93 条保留 + 17 条私有矩阵）；非人形 `BodyPlan`/`PartBoxes`/Dugu mapping 用到 `#[cfg(test)]` 的 `RaceRegistry::from_parts_for_test(_with_meridian_mappings)` 与同级合成 registry，NPC runtime 用 `pub(crate) spawn_test_npc_runtime_shape`；倍率、prepaid、part-consequence 三组直接依赖生产私有纯函数且无公开纯函数入口。其余 ECS/武器/盾/护甲/VFX/anticheat cases 依赖已登记的同级 fixture 组合来锁住完整副作用链，不新增 public、`pub(crate)` 或 `#[doc(hidden)]` seam，也不复制生产实现；能用公开入口验证的 lifecycle/reach/mode cases 已移出。
- **真元守恒范围纠偏（方案 A）**：第一性核验确认基线攻击投入路径在合法命中结算阶段仅改 `Cultivation.qi_current`，未构造或消费 canonical ledger transfer；本 PR 复审中已移除曾临时加入的攻击投入现场结算逻辑，因为它属于生产行为且会把测试迁移 PR 越界为 gameplay 修复。规范的后续修复入口是 `qi_physics::release::qi_release_to_zone`，该缺陷另交独立修复 PR。本 PR 仅保留截脉格挡既有守恒测试：以 `SPIRIT_QI_TOTAL` 初始化预算，拍 before/after 全量 player/zone/container/ledger，调用 `qi_physics::ledger::assert_conservation`，并显式构造完整 `qi_physics::ledger::QiTransfer { from, to, amount, reason }` 与 Event/`WorldQiAccount` 审计轨迹对拍；攻击方使用已预付的 `BurstMeridian` 仅作为非物理命中触发器，以隔离该既有缺陷。仓库不存在名为 `QiLedger` 的类型；`WorldQiAccount::transfer` 不能重复镜像 external player/zone，否则会双计 zone。
- **生产边界与落点**：`server/src/combat/resolve.rs` 仅保留 `#[cfg(test)] #[path = "resolve_tests.rs"] mod tests;`，公开 gate 在 `server/tests/unit/combat/resolve_test.rs`，由显式 `resolve_unit` target 发现。恢复基线攻击投入语义，不改伤害公式、部位倍率、命中判定、暴击、护甲减免、玩法、schema、wire 或其它迁移范围；攻击投入未入 canonical ledger 的既有缺陷不由本 PR 修复。
- **主线合并后复验（代码 HEAD `2eac63197`）**：紧邻执行 `git fetch origin && git merge origin/main`，基于 `origin/main=4dd7bc17e` 无冲突生成合并提交；锁内定向 `cargo test combat::resolve::tests --lib` 为 `96 passed / 0 failed / 0 ignored`，外置 `cargo test --test resolve_unit` 为 `11 passed / 0 failed / 0 ignored`。不以迁移前后测试数量守恒作验收。完整 locked server gate（`fmt --check`、`clippy --all-targets -- -D warnings`、`cargo test`）exit `0`：library `12174 passed / 0 failed / 1 ignored`，main `18 passed / 0 failed / 0 ignored`；`client_payload_unit=9`、`proto_gen_unit=55`、`resolve_unit=11`、`shader_state_unit=7`、`skin_packet_unit=2`、`inventory_unit=40` 及其余已注册 targets 均无失败，doc-tests `3 passed / 0 failed / 5 ignored`。代码未改伤害/qi/协议运行时语义；Kody 当前 HEAD 主动结论与 CI/e2e 结果在 PR 收口后补入。

## 验收抓手（T0）

- `docs/plan-test-layout-refactor-v1.md` 为当前 active plan，`scripts/test-all.sh`、`scripts/test-all-owners.tsv` 与 `scripts/tests/test_all_contract_test.sh` 已按 P1 交付。
- 盘点覆盖三栈及根 preview/contract tooling 的 source directory、local command、CI job、native report/artifact；每行都有 owner 和 consumer。
- 矩阵明确区分测试语义 owner、编排 owner、报告 producer/consumer；P1 交付的 `scripts/test-all-owners.tsv` 必须能逐行核验 suite/path/evidence 三列，且声明跨栈 smoke 不得复制场景。
- P2 测试准入策略已冻结：协议/安全/守恒/状态转换/兼容与真实回归保留；实现镜像、fixture 文案、内部顺序和源码文本扫描须分类收缩。后续 P4 的 `inline-test-inventory.tsv` 是处置审计依据，不以测试数量归零验收。
- `git diff --name-only`（在 T0 干净基线核验时）只应出现当时的 skeleton 文件；P1 本轮不要求清理工作树中已有的用户文件。
- 本轮不修改任何 `server/**`、`client/**`、`agent/**` 测试路径，不修改 `.github/workflows/**`，不添加依赖；外部 BongWorldGen 不在本 PR 的路径/命令/CI 变更范围内。

## §8 开放问题（升 active / P1 决策门前需收口）

1. **`unit` profile 是否包含根 `scripts/tests/**` 和 asset/model Python tests**：建议保留三栈 unit 与 `scripts` contract 分层，避免“全量”名字掩盖跨栈副作用；需 CI/DevEx owner 确认默认本地时限。
2. **统一入口是否并行**：server/client 编译并发、共享 Cargo target、Gradle daemon 和 preview run-private 输出的资源上限需用一轮实测决定；在此之前只承诺 DAG，不承诺并行度。
3. **Vitest/根 Python validator 是否引入统一 JUnit reporter**：现状 schema/tiandao 依赖 stdout，`scripts/preview` validator 也没有统一 reporter；需先确认 GitHub artifact/检查器是否真正消费 JUnit，不能为格式统一而新增无消费者产物。
4. **CI 接入策略**：shadow run 的 job 选择、重复执行预算和失败归因窗口需要拍板；未经对拍不得删除现有显式命令。
5. **artifact retention 与命名是否冻结为现状**：`evidence-*`、`schema-dist`、`bong-server-release`、`bong-resourcepack-*` 已被 job/PR comment 消费，保留期和命名变更需 CI/DevEx 与各栈 owner 共同决议；不恢复 `worldgen-snapshot-*`。

### §8.1 决议要求（pre-P1）

原开放问题全部在本节收口；原表保留以便追溯，实施时以以下决议为准。

### #1 `unit` profile 的范围

**决议**：`unit` 只运行 `server`、`client`、`schema`、`tiandao` 四个 canonical suite；根 `scripts/tests/**`、asset/model validator 属于 `contract`，不因名字相似混入 unit。`--profile contract` 才运行脚本合同、resourcepack 和 preview validator。

**边界**：`scripts/smoke-*.sh`、bot/chat E2E、Redis、真实 LLM 和外部 BongWorldGen 永不由 unit 隐式启动。

**落点**：`scripts/smoke-test.sh:16-23,45-59`（现有栈命令）/ 本文 `P1 CLI 契约`、`P3 CI 兼容`。

### #2 统一入口并行策略

**决议**：P1 采用串行、可复现的 suite DAG；只在 P3 shadow 对拍收集实测数据后，才允许在 server/client/agent 之间并行。共享 Cargo target、Gradle daemon、run-private 报告目录和 preview 输出必须各有资源锁。

**边界**：并行失败、工具缺失或资源锁冲突必须是明确 `FAIL`/`SKIP`，不能通过后台进程或 `tail` 吞退出码。

**落点**：`server/Cargo.toml:71-84`、`client/build.gradle:202-216` / 本文 `P1 编排与报告契约`、`P3 CI 兼容`。

### #3 JUnit/coverage 转换

**决议**：P1-P3 不新增 JUnit/coverage reporter。统一 envelope 只索引原生 Cargo stdout、Gradle XML/HTML、Vitest stdout 和脚本日志；只有存在实际 artifact consumer 且对拍证明能改善失败诊断时，才另立 CI/DevEx PR。

**边界**：不得修改 schema/tiandao `npm test`、Gradle `runGametest` 或 Cargo 测试输出以适配格式转换。

**落点**：`agent/packages/schema/package.json:19-24`、`agent/packages/tiandao/package.json:7-14`、`client/build.gradle:202-216` / 本文 `P1 编排与报告契约`、`P3 CI 兼容`。

### #4 CI shadow 策略

**决议**：P3 只在 `.github/workflows/e2e.yml:149-187` 的 `server-test` 做第一阶段 shadow，命令为 `test-all.sh --profile unit --suite server` + 原 `cargo test`；保留 `needs`、timeout、`evidence-server-test` 与下游 artifact 语义。连续至少一轮主线 CI 对拍且退出码、计数、失败归因一致后，才能切换 job；其他 job 按同样模板逐个接入。

**边界**：不删除原命令、不改 artifact 名称、不恢复外部 worldgen job；任何差异先回到 owner review。

**落点**：`.github/workflows/e2e.yml:149-187`、`.github/workflows/e2e.yml:87-147` / 本文 `P3 CI 兼容`。

### #5 artifact 命名与保留

**决议**：沿用现有 `schema-dist`、`bong-server-release`、`evidence-client`、`evidence-schema`、`evidence-agent`、`evidence-server-test`、`evidence-smoke`、`evidence-bot-shard-*`、`evidence-chat-window` 名称和 producer/consumer 关系；P1-P4 不改 retention，也不把统一 envelope 当成新 artifact 的替代品。

**边界**：新增 summary 只放 run-private 目录并由调用方显式上传；无消费者的产物不加入 CI，外部 raster 仍只读 handoff。

**落点**：`.github/workflows/e2e.yml:81-187,248-254,304-312,369-373,421-428` / 本文 `T0 产物矩阵`、`P1 编排与报告契约`。

以上决议已用于 P1；skeleton 已按单独 promotion commit 移为 `docs/plan-test-layout-refactor-v1.md`，P1 交付等待测试/CI owner 审阅。任何未来任务卡必须附带下列“Test Refactor 附录”。

## 后续任务卡固定附录（复制模板）

```text
Test Refactor 附录（plan-test-layout-refactor-v1）
- 测试位置：<server/tests/unit/** | server/tests/** | server/src/**/tests.rs（仅登记例外） | client/src/test/** | client/src/gametest/** | agent/packages/*/tests/** | scripts/tests/**>
- 禁止：不得在生产实现文件新增测试体；不得为搬迁新增仅供测试调用的 pub/doc-hidden seam；不得把不同栈测试合并到顶层 test/。
- 私有契约：若必须使用 `tests.rs`，写明公开 API 为何不足、外置为何会扭曲生产 API，以及复审条件；先评估公开可观察行为，不以新 seam 作为默认解法。
- 分类：为每个测试或同构测试组记录受保护契约、风险与处置（保留/合并/替换/删除）。协议、权限、守恒、状态转换、兼容与真实回归不可因收缩而丢失；字段数、内部顺序、fixture 文案和源码文本扫描默认删除或改为行为断言。
- 验证：运行受影响栈完整门禁，并附受保护契约、失败分支和外部报告证据；测试数量变化只需能由分类记录解释。
- 大规模移动：拆成独立迁移 PR，不与业务行为变更混合。
```
