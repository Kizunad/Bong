# plan-ci-redis-pull-resilience-v1（骨架）

> **骨架（草案）**。一句话主题：e2e CI 的 "Bring up Redis test service" 步频繁因 Docker Hub registry 超时失败（infra flake），加重试 / 预拉缓存 / GHCR mirror 提升 CI 韧性。

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

## 审计来源

worldgen-v4 + skeleton 实现期多个 PR 的 e2e Docker Hub flake 频发（#561/#562/#563 实证）。**report-only**，CI infra 改进。
