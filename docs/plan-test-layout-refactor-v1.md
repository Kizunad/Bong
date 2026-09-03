# plan-test-layout-refactor-v1 — 三栈测试外置布局、所有权冻结与统一入口设计

> **一句话主题**：保留 server/client/agent 各自的 canonical 测试根，禁止新增生产文件内联测试，并按可回滚的小批次把现有 Rust inline tests 外置；同时冻结执行命令、CI job 与报告产物所有权，落地 `scripts/test-all.sh` 统一入口。
>
> **当前状态**：Active plan；T0 设计盘点与“独立测试根 + 分阶段外置”决议已完成，P1 统一入口/owners/report contract 已交付；P2 首批 pseudo-vein、tsy_container_search、lingtian range_gate、craft recipe 与 alchemy furnace 模块已完成，后续模块与 P3-P4 尚未开始。
>
> **盘点基线**：2026-08-23，专用 worktree `.agent-worktrees/test-refactor-init`（分支 `plan-test-layout-refactor-v1`，基于 `origin/main=eea1e73f2`）。数量是目录扫描快照，不是测试用例总数；新增测试后以本矩阵的路径/命令契约为准。外部 `BongWorldGen` 不属于本仓库或本 plan 的测试栈；Bong 仅保留 raster handoff 与 server/client preview 消费端。

| 阶段 | 主题 | 状态 |
|------|------|------|
| T0 / P0 | 三栈盘点、CI/产物地图、所有权矩阵冻结、统一入口契约设计 | ✅ 2026-08-23 |
| P1 | 测试放置规则、禁止新增 inline、`scripts/test-all.sh` 与 owners 映射层 | ✅ 2026-08-28 |
| P2 | Rust inline tests 首批按模块外置到 `server/tests/unit/**` 或专用测试文件 | ⏳ |
| P3 | CI 兼容接入、报告收口与迁移前后对拍 | ⬜ |
| P4 | 剩余 Rust inline tests 分批外置、清点归零、全量回归 | ⬜ |

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
| Rust | `server/tests/unit/**`、`server/tests/**`、`server/benches/**` | 不得在 `server/src/**` 新增 `#[cfg(test)] mod tests` 测试体；优先测试公开行为 | 按模块小批次迁移到 `server/tests/unit/**`；若确实依赖私有实现，先抽取稳定的 test seam/fixture，否则使用与模块同目录的专用 `tests.rs`，不得继续内联在生产文件 |
| Fabric | `client/src/test/java/**`、`client/src/test/resources/**`、`client/src/gametest/java/**` | 测试类、fixture、GameTest 分别放在既有 source set；生产 Java 不放测试方法 | 现有外置路径保持不变，仅在业务触及该模块时整理命名/目录 |
| Agent | `agent/packages/schema/tests/**`、`agent/packages/tiandao/tests/**` | Vitest 用例、samples/generated 对拍均放包级测试目录；生产 `src/**` 不新增测试体 | 现有外置路径保持不变，不把 schema 与 tiandao 合并成单一目录 |
| 根脚本 | `scripts/tests/**`、`scripts/preview/**`、显式 smoke/E2E 脚本 | contract/validator 与跨栈场景留在脚本根；不得复制到三栈目录 | 只整理归属和报告索引，不改变脚本的前置、Redis、时间或 artifact 语义 |

### 不变式与边界

1. **新测试先外置**：所有后续业务 PR 的任务卡必须附带本节；若测试代码进入生产文件，PR body 必须说明私有访问原因、替代方案评估和清理期限，否则视为不符合 plan。
2. **迁移不改行为**：迁移只改变测试 source path、fixture 所有权和构建发现方式；断言、协议、Redis key、时间/随机种子和产物命名不得借机调整。
3. **不机械破坏私有边界**：Rust 集成测试无法访问私有符号时，不通过扩大可见性或复制实现来“迁移”；先提取最小稳定 test seam，仍无法合理抽取时保留专用 `tests.rs` 并记录理由。
4. **业务与搬迁分离**：大批量文件移动单独成 PR；业务修复 PR 只新增外置测试或迁移同一模块所需的最小用例，避免 review 混入无关 rename。
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
| Rust unit/integration/bench | 迁移前 `server/src/**` + `server/tests/**`；目标 `server/tests/unit/**`、`server/tests/**`、`server/benches/**` | Server | Server owner；CI job 仅收集 | 新测试不得进入生产文件；迁移 PR 只移动测试与必要的最小 test seam，不由 wrapper 重写断言 |
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

## P2 — Rust inline tests 首批外置（⏳）

P2 按模块拆成多个原子 PR，每个 PR 只迁移一个模块的测试，并保留原测试名、fixture、随机种子和断言语义。首批优先选择 `server/src/world/pseudo_vein_runtime.rs:793-1332`、`server/src/world/tsy_container_search.rs:757-1661` 等边界清晰的模块；具体名单以迁移前 grep 快照为准。

- **目标路径**：`server/tests/unit/<module>_test.rs`，由 `server/src/lib.rs` 暴露的公开 API 驱动；生产文件不得新增测试模块声明。
- **私有访问**：先抽取最小、稳定、非 gameplay 的 test seam/fixture；不为迁移扩大整个模块可见性，不复制生产实现。无法合理抽取的遗留例外必须在 PR body 和 plan 迁移表登记，并单独安排清理。
- **验收**：迁移前后定向 `cargo test <filter>` 测试数量、失败行为和产物一致；完成后跑 server 完整 fmt/clippy/test 门禁。

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

## P3 — CI 兼容、报告收口与迁移对拍（⬜）

- 先在 `.github/workflows/e2e.yml` 的 `server-test` job 进行 shadow run：`test-all.sh --profile unit --suite server` 与原 `cargo test` 并行执行，保留原命令、DAG、`evidence-server-test` artifact 和超时。
- 对拍至少覆盖退出码、测试计数、失败 suite、原生 Cargo 输出和 `.sisyphus/evidence/**`；差异必须归因后才能切换 job 命令。
- Client/Schema/Tiandao 仅接入其已有 canonical path 和原生命令，不借 P3 改源测试目录；统一报告只索引 Gradle/JUnit、Cargo、Vitest 和脚本原生产物，不强制新增无消费者的 JUnit 转换。

## P4 — 剩余 Rust inline tests 分批外置与归零（⬜）

- 根据 P0 grep 快照维护 `inline-test-inventory.tsv`：模块、测试数量、目标路径、私有 seam、责任人、迁移 PR、对拍命令、状态。
- 以一个模块一个 PR 的节奏迁移剩余 `server/src/**` 测试；每个 PR 不混入业务行为变化、无关格式化或跨模块 rename。
- 归零门禁：生产源码中除明确登记的临时例外外，不再出现 `#[cfg(test)]` 测试体；`scripts/test-all.sh --profile unit --suite server`、原生 Cargo 门禁和 server CI 全绿。
- P4 完成后才能将本 plan 全阶段标记 ✅；没有“默认无需搬路径”的回退路线。

## 验收抓手（T0）

- `docs/plan-test-layout-refactor-v1.md` 为当前 active plan，`scripts/test-all.sh`、`scripts/test-all-owners.tsv` 与 `scripts/tests/test_all_contract_test.sh` 已按 P1 交付。
- 盘点覆盖三栈及根 preview/contract tooling 的 source directory、local command、CI job、native report/artifact；每行都有 owner 和 consumer。
- 矩阵明确区分测试语义 owner、编排 owner、报告 producer/consumer；P1 交付的 `scripts/test-all-owners.tsv` 必须能逐行核验 suite/path/evidence 三列，且声明跨栈 smoke 不得复制场景。
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
- 测试位置：<server/tests/unit/** | server/tests/** | client/src/test/** | client/src/gametest/** | agent/packages/*/tests/** | scripts/tests/**>
- 禁止：不得在生产文件新增 #[cfg(test)]/测试体；不得把不同栈测试合并到顶层 test/。
- 若必须 inline：写明私有访问原因、已评估的 test seam、清理期限，并在 PR body 登记例外。
- 迁移边界：只改变测试路径/所有权，不改变行为、协议、Redis key、时间/随机种子或报告 artifact。
- 验证：运行受影响栈完整门禁，并附测试数量/失败分支/外部报告证据。
- 大规模移动：拆成独立迁移 PR，不与业务行为变更混合。
```
