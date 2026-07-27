# plan-ci-redis-pull-resilience-v1

> **复核记录（2026-07-27）**：本 plan 曾于同日在 PR #1291 初版归档为「P0 文档追认已实装 + P1 撤回」100% merged。经 `/review` 复审（4 个 reviewer × 2 轮，8/8 判定 misaligned）指出两条问题：① P1 撤回所用的 e2e run 样本未限定立项目标所指的 `pull_request` event；② §N.1 #2 已确认 CI gate 路径上存在 `scripts/bot-e2e.sh:138-139` 的 Redis 拉取分支且无重试，归档材料却只把它登记为「供后续认领」的遗留项，随后仍按 100% 归档——**登记不等于闭环**。两条均属实，**归档已撤销**：plan 移回 active（`git mv docs/finished_plans/... docs/...`），阶段总览新增 P2 承接第②条，§N.1 #1 按第①条要求补做 `pull_request` event 过滤后重新验证（结论不变，证据口径更严，见下）。

> 立项动机：worldgen-v4 + 审阅 skeleton 连续多个 PR（#561/#562/#563）的 PR-event e2e run 在 "Bring up Redis test service"（`docker compose -f docker-compose.test.yml up -d redis --wait`）死于 `Error Get "https://registry-1.docker.io/v2/": net/http: request canceled ... Client.Timeout exceeded`。同 tip 的 workflow_dispatch e2e 常 success（Docker Hub 间歇性），但 PR check 红逼迫 --admin 合并，掩盖真实 gate。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | redis 镜像拉取加重试（docker compose pull 失败重试 N 次 + 退避） | ✅ 2026-07-27（实装早于本 plan 归档尝试：`d668ae85b` #575，2026-06-15；本条为文档追认，见「P0 落地记录」） |
| P1 | 镜像源韧性（GHCR/ECR mirror 或 actions cache 预拉 redis 镜像） | ⬜ 撤回（2026-07-27，见 §N.1 #1：`pull_request` event 过滤后重试仍充分，不实施） |
| P2 | CI gate 路径上其余 redis 拉取点韧性（`scripts/bot-e2e.sh:138-139`） | ⬜ 未开工（2026-07-27 由 review 拆出，见 §N.1 #2 / 下方「P2」小节） |

## 接入面 checklist

- **落点**：`.github/workflows/e2e.yml`（"Pre-pull Redis image with retry" / "Bring up Redis test service" step）+ `docker-compose.test.yml`（redis service image）+ P2 涉及 `scripts/bot-e2e.sh`。
- **跨仓库契约**：纯 CI infra，不动 server/client/agent 运行时。
- 参 memory `project_snapshot_ci_broken`（CI env 缺口历史）。

## P0 — 拉取重试

- "Bring up Redis test service" 前加 `docker compose pull redis` with retry（如 3 次 + 指数退避），或用带 retry 的 action；避免单次 Docker Hub 超时即整个 e2e 红。

**落地记录**：`.github/workflows/e2e.yml:104-114`「Pre-pull Redis image with retry」step（3 次 attempt + `attempt*10` 秒线性退避 + 循环耗尽后末尾兜底再拉一次），紧接其后的「Bring up Redis test service」（`:116-117`）实际消费该预拉结果。实装 commit：`d668ae85b`「ci: 为 e2e Redis 镜像拉取加重试 (#575)」（2026-06-15）——早于本 plan 的归档/复核尝试，本次改动未对 `.github/workflows/e2e.yml` 或 `docker-compose.test.yml` 做任何修改（纯文档 PR）。

## P1 — 镜像源韧性

- redis 镜像改用 GHCR mirror（`ghcr.io/.../redis`）或 actions/cache 预缓存镜像 layer，减少对 Docker Hub registry 的实时依赖。

见 §N.1 #1 决议：撤回，不实施。

## P2 — CI gate 路径上其余 redis 拉取点韧性

**现状**（`scripts/bot-e2e.sh:137-141`）：

```bash
if ! port_open 127.0.0.1 6379; then
  echo "[bot-e2e] redis 未起，尝试 docker compose 拉起"
  docker compose -f "$ROOT/docker-compose.test.yml" up -d redis --wait
  STARTED_REDIS=1
fi
```

该分支由 `e2e.yml:151`「Bot e2e stage (protocol-level player scenarios)」直接调用 `bash scripts/bot-e2e.sh`，因此位于 e2e CI gate 路径上；`docker compose ... up -d redis --wait` 这一行本身**无重试**——如果被触发且当次撞上 Docker Hub 间歇性超时，会重现本 plan 立项动机描述的同类 flake。

**控制流分析（2026-07-27 复核补充，降低但不消除风险）**：

- `port_open 127.0.0.1 6379` 的判断依赖同一 job 内更早的「Bring up Redis test service」（`e2e.yml:116-117`）已经把 `docker-compose.test.yml` 定义的 redis 服务起在固定端口 `6379:6379`（`docker-compose.test.yml:6-7`）；该服务直到 job 末尾「Tear down Redis test service」（`e2e.yml:170-172`，`if: always()`）才会被 `down -v --remove-orphans`，中间没有任何步骤对它做 `down`。因此在**正常执行路径**下，跑到「Bot e2e stage」时 `port_open` 应恒为真，`bot-e2e.sh:138` 这行**不会被执行**——`scripts/e2e-redis.sh` 里同款的 `ensure_redis()`（`:800-804`）同样先 `probe_redis`，复用已起实例才 fallback 到 `start_docker_redis`（`:790-798`），是同一模式。
- 即使假设性地被触发（例如 redis 容器中途异常退出），`docker compose up` 默认 `pull_policy: missing`——只在本地找不到镜像时才会去 registry 拉取；而同一 job 更早的「Pre-pull Redis image with retry」（`e2e.yml:104-114`）已经把 `redis:7-alpine` 拉进本地 Docker daemon 缓存，GitHub Actions 单个 job 全程共享同一 runner / 同一 Docker daemon，且已核实 `.github/` + `scripts/` 全仓没有 `docker rmi` / `docker system prune` 之类会清掉该镜像的调用。所以就算这一行被执行，大概率也只是复用本地缓存镜像，不会真正打到 `registry-1.docker.io`。
- **但以上两条都是静态控制流推理，不是故障注入实测**——没有真实构造过「redis 容器中途消失、bot-e2e.sh fallback 分支被迫触发」的场景来验证它的行为，也没有验证过本地镜像若真的缺失时这条路径会不会像最初的 flake 一样卡死整个 e2e gate。这正是 review 判定「未闭环」的依据，**不能用推理代替实测直接销掉这一条**，故仍按开放阶段处理。

**要落什么**：给 `scripts/bot-e2e.sh:138-139` 补上与 P0 同款的重试（3 次 + 线性退避），或用故障注入/实测证明该分支在当前 CI 控制流下不可能触达 registry 因而不需要重试。`scripts/e2e-redis.sh:792`（`docker run redis:7-alpine`，`start_docker_redis`）是本地 harness 用的同款兜底，同一风险模式但优先级低于 CI gate 路径，可一并处理或留待下一阶段。

**验收怎么算**：

1. 目标脚本改动后，实际触发一次让 fallback 分支执行的 e2e run（或等价的故障注入：临时让「Bring up Redis test service」步骤失败/让 redis 提前退出），确认新加的重试确实生效；或
2. 提供比上面「控制流分析」更强的证据——例如真实注入「Bring up Redis test service 失败」后观察 `bot-e2e.sh` fallback 是否触发、触发后是否需要访问 registry——证明该路径在当前 CI 控制流下不可能造成 registry 依赖；
3. 二者择一完成后，P2 才可标 ✅，本 plan 才可重新归档。

## §N 开放问题

1. 重试（P0，轻）够不够，还是需 mirror（P1，重）。
2. 是否其他 CI job 也拉 Docker Hub 镜像（统一加韧性）。

> #1 已在 §N.1 收口。#2 已拆出为上方 P2 阶段，尚未收口——见 §N.1 #2。

## §N.1 决议

### #1 重试够不够，还是需要 mirror（收口，2026-07-27；证据 2026-07-27 复核按 `pull_request` event 重新验证）

**决议**：

1. **重试已充分，不实施 P1 GHCR/ECR mirror。**
2. **依据（2026-07-27 复核，按立项动机所指的 `pull_request` event 过滤）**：

   ```bash
   gh run list --workflow=e2e.yml --limit 100 --json event,conclusion,databaseId,createdAt \
     --jq '[.[] | {event, conclusion}] | group_by(.event) | map({event: .[0].event, n: length, fail: ([.[]|select(.conclusion=="failure")]|length)})'
   ```

   最近 100 次 e2e.yml run 按 event 分层：`pull_request` 85 次（7 次 failure）、`push` 15 次（0 次 failure）；样本窗口内**没有 `workflow_dispatch` run**。单独取出 `pull_request` 子样本（`--jq 'select(.event=="pull_request")'`）：**85 次，窗口 2026-07-22T08:24:22Z ~ 2026-07-26T15:54:27Z（约 4.3 天），7 次 failure**。逐个 `gh run view <id> --json jobs --jq '.jobs[] | .steps[] | select(.conclusion=="failure") | .name'` 核实失败 step：`Bot e2e stage (protocol-level player scenarios)` ×3、`Smoke/E2E stage (Task 13 harness)` ×2、`Server stage (cargo test)` ×2——**零次**发生在 `Pre-pull Redis image with retry` 或 `Bring up Redis test service`。
   85 是足够支撑结论的样本量（不下调结论强度）；且本次是专门针对 `pull_request` event 的子样本，比初版归档使用的未过滤混合样本（`--limit 100` 未传 `--event`，可能混入非 PR-event run）更贴合立项目标所指的 PR gate。
3. **证据边界（不作过度外推）**：上述窗口只有约 4.3 天，**不覆盖 P0（`d668ae85b` #575，2026-06-15）落地至今的全程**——GitHub 的 run 列表在本仓当前 run 密度下 100 条即回溯到 07-22，再往前需要分页翻历史，本次未做。因此本决议的成立范围是「**在已观测的 85 次 `pull_request`-event / 约 4.3 天窗口内，redis 拉取零失败**」，**不宣称**「P0 以来从未复现」。
4. P1 不进入实施队列；若未来 e2e 历史重新出现 redis-pull 相关失败，须凭新的实测数据另立评估（不在本 plan 复活范围内）。

**落点**：`.github/workflows/e2e.yml:104-114`（P0 实装）；本 plan §阶段总览 P1 行。

### #2 是否其他 CI job 也拉 Docker Hub 镜像（未收口，2026-07-27 拆出 P2）

**决议**：

1. **本 plan 的 P0 范围不扩面，但「全仓只有一处 Docker Hub 依赖」这个说法是错的**——2026-07-27 复核时把搜索面从 `.github/workflows/*.yml` 扩到全仓，找到多处此前漏掉的拉取点，逐一登记于下。
2. **初版依据的缺陷（诚实记录）**：初版只跑了 `grep -rn "docker compose\|docker pull\|image:" .github/workflows/*.yml` 与 `grep -rn "services:" .github/workflows/*.yml`，据此下了「全仓对 Docker Hub 的实时依赖点只有 e2e.yml 一处」的结论。**workflow 目录的 grep 覆盖不了被 workflow 调用的 shell 脚本**，结论宽于证据。
3. **全仓实际拉取点**（`grep -rIn -E "docker (compose|pull|run|build)|^\s*image:\s*\S|FROM " .` 排除 `.git/target/build/node_modules/.gradle`，并 `find` 全部 Dockerfile / compose 文件）：
   - `.github/workflows/e2e.yml:104-117` —— **已被 P0 重试覆盖**
   - `scripts/bot-e2e.sh:138-139` —— `docker compose ... up -d redis --wait` 兜底路径，**在 CI gate 路径上**（`e2e.yml:151` 直接 `bash scripts/bot-e2e.sh`），**无重试** → **拆为本 plan P2 阶段**（见上方「P2」小节），不再作为无人认领的遗留项悬空
   - `scripts/e2e-redis.sh:792`、`scripts/e2e-offscreen-war.sh:1458`、`scripts/smoke-offscreen-war.sh:121`、`scripts/e2e-chat-signal-window.sh:94` —— 各自 `docker run ... redis:7-alpine`，本地 harness 用，不在 e2e CI gate 路径上，优先级低于 P2，P2 落地时可一并处理
   - `library-web/Dockerfile:1,26`（`node:20-alpine` / `nginx:alpine`）、`library-web/docker-compose.yml:40`（`cloudflare/cloudflared:latest`）—— 静态站点部署侧，不在 e2e gate 路径，不属于本 plan 范围
   - `grep -rn "services:\|container:" .github/workflows/` 仍为 0 命中——**这一条初版没错**，确无 GitHub Actions 托管容器块。
4. **为何不再用「登记为遗留项」结案**：初版归档把 `scripts/bot-e2e.sh:138-139` 写成「供后续 plan 认领」的遗留项，然后仍按 100% 归档——review 指出这是「登记不等于闭环」。本次复核改为**在本 plan 内新增 P2 阶段**直接承接，不外抛给未来某个不存在的 plan/issue；P2 未 ✅ 之前，本 plan 不算收口。

**落点**：`.github/workflows/e2e.yml:104-117`（P0 覆盖面）；`scripts/bot-e2e.sh:138-139`（P2 阶段目标）；本 plan「P2」小节。

---

§N #1 已在 §N.1 收口。§N #2 已拆为 P2 阶段，**尚未收口**——本 plan 因此保持 active，不得归档。

## 审计来源

worldgen-v4 + skeleton 实现期多个 PR 的 e2e Docker Hub flake 频发（#561/#562/#563 实证）。**report-only**，CI infra 改进。
