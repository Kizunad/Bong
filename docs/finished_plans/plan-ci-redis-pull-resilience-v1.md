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
2. 依据（**2026-07-27 复核时已把样本从 25 次扩到 100 次，并显式标注窗口边界**）：`gh run list --workflow=e2e.yml --limit 100` 取到的 100 次 run 覆盖 **2026-07-22 ~ 2026-07-26**，其中 6 次 `conclusion=="failure"`。逐个跑 `gh run view <id> --json jobs --jq '.jobs[] | .steps[] | select(.conclusion=="failure") | .name'`，失败 step 分布为：`Bot e2e stage (protocol-level player scenarios)` ×3、`Smoke/E2E stage (Task 13 harness)` ×2、`Server stage (cargo test)` ×1——**零次**发生在 `Pre-pull Redis image with retry` 或 `Bring up Redis test service`。
3. **证据边界（不作过度外推）**：上述窗口只有约 5 天，**不覆盖 P0（`d668ae85b` #575，2026-06-15）落地至今的全程**——GitHub 的 run 列表在本仓当前 run 密度下 100 条即回溯到 07-22，再往前需要分页翻历史，本次未做。因此本决议的成立范围是「**在已观测的 100 次 / 5 天窗口内，redis 拉取零失败**」，**不宣称**「P0 以来从未复现」。
4. P1 不进入实施队列；若未来 e2e 历史重新出现 redis-pull 相关失败，须凭新的实测数据另立评估（不在本 plan 复活范围内）。

**落点**：`.github/workflows/e2e.yml:104-114`（P0 实装）；本 plan §阶段总览 P1 行。

### #2 是否其他 CI job 也拉 Docker Hub 镜像

**决议**：
1. **本 plan 的 P0/P1 范围不扩面，但「全仓只有一处 Docker Hub 依赖」这个说法是错的**——2026-07-27 复核时把搜索面从 `.github/workflows/*.yml` 扩到全仓，找到多处此前漏掉的拉取点，逐一登记于下，并把 CI 路径上仍无重试的那处列入「遗留 / 后续」。
2. **初版依据的缺陷（诚实记录）**：初版只跑了 `grep -rn "docker compose\|docker pull\|image:" .github/workflows/*.yml` 与 `grep -rn "services:" .github/workflows/*.yml`，据此下了「全仓对 Docker Hub 的实时依赖点只有 e2e.yml 一处」的结论。**workflow 目录的 grep 覆盖不了被 workflow 调用的 shell 脚本**，结论宽于证据。
3. **全仓实际拉取点**（`grep -rIn -E "docker (compose|pull|run|build)|^\s*image:\s*\S|FROM " .` 排除 `.git/target/build/node_modules/.gradle`，并 `find` 全部 Dockerfile / compose 文件）：
   - `.github/workflows/e2e.yml:104-117` —— **已被 P0 重试覆盖**
   - `scripts/bot-e2e.sh:138-139` —— `docker compose ... up -d redis --wait` 兜底路径，**在 CI 路径上**（`e2e.yml:151` 直接 `bash scripts/bot-e2e.sh`），**无重试**
   - `scripts/e2e-redis.sh:792`、`scripts/e2e-offscreen-war.sh:1458`、`scripts/smoke-offscreen-war.sh:121`、`scripts/e2e-chat-signal-window.sh:94` —— 各自 `docker run ... redis:7-alpine`，本地 harness 用，无重试
   - `library-web/Dockerfile:1,26`（`node:20-alpine` / `nginx:alpine`）、`library-web/docker-compose.yml:40`（`cloudflare/cloudflared:latest`）—— 静态站点部署侧，不在 e2e gate 路径
   - `grep -rn "services:\|container:" .github/workflows/` 仍为 0 命中——**这一条初版没错**，确无 GitHub Actions 托管容器块。
4. **为何仍不扩进本 plan**：本 plan 的 P0 交付物原文限定为「e2e CI 的 *Bring up Redis test service* 步」，P1 为「镜像源韧性 / mirror」；上述其余拉取点从立项起就不在两阶段范围内。**但它们是真实的、未被覆盖的同类风险**，故不以「无需扩面」结案，改为登记进 Finish Evidence 的「遗留 / 后续」，供后续 plan 认领。

**落点**：`.github/workflows/e2e.yml:104-117`（P0 覆盖面）；`scripts/bot-e2e.sh:138-139`（CI 路径上未覆盖，遗留）；本 plan Finish Evidence「遗留 / 后续」。

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
- **§N #2 复核改判（2026-07-27）**：初版结论「全仓只有 e2e.yml 一处 Docker Hub 依赖 → 无需扩面」**是错的**，因为 grep 只覆盖了 `.github/workflows/*.yml`、漏掉被 workflow 调用的 shell 脚本。全仓实际拉取点见 §N.1 #2 第 3 条。其中**唯一在 CI gate 路径上却仍无重试**的是 `scripts/bot-e2e.sh:138-139` 的 `docker compose ... up -d redis --wait` 兜底（由 `e2e.yml:151` 调用）——本 plan P0/P1 原文范围不含它，故不在本 plan 内实施，**登记为后续认领项**：给该兜底路径补同款重试（或让它复用 e2e.yml 已预拉的镜像）。
- 其余拉取点（`scripts/e2e-redis.sh:792` / `scripts/e2e-offscreen-war.sh:1458` / `scripts/smoke-offscreen-war.sh:121` / `scripts/e2e-chat-signal-window.sh:94` 的 `docker run redis:7-alpine`）是本地 harness 路径，不影响 PR gate，优先级低于上一条。
- `library-web/` 的 `Dockerfile` / `docker-compose.yml`（`node:20-alpine` / `nginx:alpine` / `cloudflare/cloudflared:latest`）属静态站点部署侧，与 e2e gate 无关，仅登记不认领。
