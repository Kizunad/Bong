# plan-ci-redis-pull-resilience-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：e2e CI 的 "Bring up Redis test service" 步频繁因 Docker Hub registry 超时失败（infra flake），加重试 / 预拉缓存 / GHCR mirror 提升 CI 韧性。

> 立项动机：worldgen-v4 + 审阅 skeleton 连续多个 PR（#561/#562/#563）的 PR-event e2e run 在 "Bring up Redis test service"（`docker compose -f docker-compose.test.yml up -d redis --wait`）死于 `Error Get "https://registry-1.docker.io/v2/": net/http: request canceled ... Client.Timeout exceeded`。同 tip 的 workflow_dispatch e2e 常 success（Docker Hub 间歇性），但 PR check 红逼迫 --admin 合并，掩盖真实 gate。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | redis 镜像拉取加重试（docker compose pull 失败重试 N 次 + 退避） | ⬜ |
| P1 | 镜像源韧性（GHCR/ECR mirror 或 actions cache 预拉 redis 镜像） | ⬜ |

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
