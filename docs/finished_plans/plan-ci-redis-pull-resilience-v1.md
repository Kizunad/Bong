# plan-ci-redis-pull-resilience-v1

> **归档（文档追认 + P1 撤回）2026-07-27**：P0（拉取重试）**早已实装**——`d668ae85b`「ci: 为 e2e Redis 镜像拉取加重试 (#575)」于 2026-06-15 落地 `.github/workflows/e2e.yml:104-114`，本 plan 文档当时未跟着更新阶段状态。本次归档 PR **不新写任何实装代码**，只是补文档追认既有代码 + 收口 §N 两个开放问题。P1（GHCR/ECR mirror）经 §N.1 #1 实测收口后**撤回，不实施**。

> 立项动机：worldgen-v4 + 审阅 skeleton 连续多个 PR（#561/#562/#563）的 PR-event e2e run 在 "Bring up Redis test service"（`docker compose -f docker-compose.test.yml up -d redis --wait`）死于 `Error Get "https://registry-1.docker.io/v2/": net/http: request canceled ... Client.Timeout exceeded`。同 tip 的 workflow_dispatch e2e 常 success（Docker Hub 间歇性），但 PR check 红逼迫 --admin 合并，掩盖真实 gate。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | redis 镜像拉取加重试（docker compose pull 失败重试 N 次 + 退避） | ✅ 2026-07-27（实装早于本 PR：`d668ae85b` #575，2026-06-15；本条为文档追认，见 Finish Evidence） |
| P1 | 镜像源韧性（GHCR/ECR mirror 或 actions cache 预拉 redis 镜像） | ⬜ 撤回（2026-07-27，见 §N.1 #1：重试已充分，不实施） |

## 接入面 checklist

- **落点**：`.github/workflows/e2e.yml`（"Bring up Redis test service" step）+ `docker-compose.test.yml`（redis service image）。
- **跨仓库契约**：纯 CI infra，不动 server/client/agent 运行时。
- 参 memory `project_snapshot_ci_broken`（CI env 缺口历史）。

## P0 — 拉取重试

- "Bring up Redis test service" 前加 `docker compose pull redis` with retry（如 3 次 + 指数退避），或用带 retry 的 action；避免单次 Docker Hub 超时即整个 e2e 红。

## P1 — 镜像源韧性

- redis 镜像改用 GHCR mirror（`ghcr.io/.../redis`）或 actions/cache 预缓存镜像 layer，减少对 Docker Hub registry 的实时依赖。

## §N 开放问题

1. 重试（P0，轻）够不够，还是需 mirror（P1，重）。
2. 是否其他 CI job 也拉 Docker Hub 镜像（统一加韧性）。

> 全部已在 §N.1 收口。原表保留以备追溯，实施时以 §N.1 决议为准。

## §N.1 决议（收口，2026-07-27）

### #1 重试够不够，还是需要 mirror

**决议**：
1. **重试已充分，不实施 P1 GHCR/ECR mirror**。
2. 依据：`gh run list --workflow=e2e.yml --limit 25 --json conclusion,databaseId,createdAt` 拉取近 25 次 e2e run（`createdAt` 2026-07-25T16:27:45Z ~ 2026-07-26T14:04:19Z），其中仅 2 次 `conclusion=="failure"`（databaseId `30202576976`、`30202432895`）。对这 2 次分别跑 `gh run view <id> --json jobs --jq '.jobs[] | .steps[] | select(.conclusion=="failure") | .name'`，唯一失败 step 均为 `Bot e2e stage (protocol-level player scenarios)`——**零次**失败发生在 `Pre-pull Redis image with retry` 或 `Bring up Redis test service` 步骤。P0（3 次 attempt + `attempt*10` 秒线性退避 + 末尾兜底再拉一次，落地于 `d668ae85b` #575，2026-06-15）以来，redis 镜像拉取超时未再复现为可观测 CI 失败。
3. P1 不进入实施队列；若未来 e2e 历史重新出现 redis-pull 相关失败，须凭新的实测数据另立评估（不在本 plan 复活范围内）。

**落点**：`.github/workflows/e2e.yml:104-114`（P0 实装）；本 plan §阶段总览 P1 行。

### #2 是否其他 CI job 也拉 Docker Hub 镜像

**决议**：
1. **无需扩面**。
2. 依据：`grep -rn "docker compose\|docker pull\|image:" .github/workflows/*.yml` 仅 `e2e.yml` 命中（`pull redis` / `up -d redis` / `logs redis` / `down`）；`grep -rn "services:" -A 4 .github/workflows/*.yml` 全仓无匹配——没有任何 workflow 使用 GitHub Actions 托管的 `services:` 容器块（该机制会绕开显式 `docker compose pull` 自行拉镜像，需要单独加固）。全仓对 Docker Hub 的实时依赖点只有 `e2e.yml` 的 redis 拉取一处，已被 P0 覆盖。

**落点**：`.github/workflows/*.yml`（全量 grep 结果，无第二处命中）；本 plan §阶段总览。

---

全部已在 §N.1 收口，实施以 §N.1 决议为准。

## 审计来源

worldgen-v4 + skeleton 实现期多个 PR 的 e2e Docker Hub flake 频发（#561/#562/#563 实证）。**report-only**，CI infra 改进。

## Finish Evidence

**落地清单**：

- P0：`.github/workflows/e2e.yml:104-114`「Pre-pull Redis image with retry」step（3 次 attempt + `attempt*10` 秒线性退避 + 循环耗尽后末尾兜底再拉一次），紧接其后的「Bring up Redis test service」（`:116-117`）实际消费该预拉结果。
- P1：未实施，撤回（见 §N.1 #1 决议）。

**关键 commit**：

- `d668ae85b`「ci: 为 e2e Redis 镜像拉取加重试 (#575)」（2026-06-15）—— P0 唯一实装 commit，早于本次归档 PR；本次归档 PR 未对 `.github/workflows/e2e.yml` 或 `docker-compose.test.yml` 做任何修改。
- 本次归档 PR 自身的 §N.1 决议 / 阶段状态 + Finish Evidence / `git mv` 三个 docs-only commit（hash 见 PR）。

**测试结果**：

- 本 PR 为**纯 docs 变更**，**未跑任何测试套件**（无 server/client/agent/worldgen 代码改动，不适用 `cargo test` / `./gradlew test` / `npm test`）。
- P0 有效性验证方式（docs-only 场景下的替代证据）：`gh run list --workflow=e2e.yml --limit 25 --json conclusion,databaseId,createdAt` 统计近 25 次 e2e run（`createdAt` 2026-07-25T16:27:45Z ~ 2026-07-26T14:04:19Z），仅 2 次 `failure`（`30202576976`/`30202432895`），经 `gh run view <id> --json jobs --jq '...'` 核实两次失败 step 均为 `Bot e2e stage (protocol-level player scenarios)`，**零次**失败发生在 redis 拉取相关 step。

**跨仓库核验**：

- 本 plan 是纯 CI infra（`.github/workflows/e2e.yml`），不触 server/client/agent 运行时，**无跨仓库 symbol 核验项**。

**遗留 / 后续**：

- P1（GHCR/ECR mirror 或 actions cache 预拉）已撤回不实施（§N.1 #1）；若未来 e2e 历史重新出现 redis-pull 相关失败，需凭新实测数据另立新 plan/骨架评估，不在本 plan 复活范围内。
- §N #2（其他 CI job 是否也拉 Docker Hub）已确认无需扩面（§N.1 #2），无后续动作。
