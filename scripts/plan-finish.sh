#!/usr/bin/env bash
# scripts/plan-finish.sh
#
# 由 Atlas 在所有 TODO 验收通过后调用（或人工手动调用）：
#   1. 校验当前在 Bong 仓库的某个 worktree/checkout 内
#   2. git mv docs/plan-<name>.md  →  docs/finished_plans/plan-<name>.md
#   3. 生成一次归档 commit（中文，匹配仓库风格）
#
# 用法：
#   bash scripts/plan-finish.sh <plan-name>
#   例：bash scripts/plan-finish.sh HUD-v1
#
# 注意：
#   - 不做 push，push 由 scripts/bong-plan-auto.sh 统一处理
#   - 若 plan 已在 finished_plans 下，视为已归档，退出 0
#   - 若 .sisyphus/plans/<name>.md 有 [BLOCKED: 标记，拒绝归档（exit 2）

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "用法: $0 <plan-name>" >&2
  exit 64
fi

PLAN_NAME="$1"
REPO_ROOT="$(git rev-parse --show-toplevel)"
PLAN_ACTIVE="$REPO_ROOT/docs/plan-$PLAN_NAME.md"
PLAN_ARCHIVED="$REPO_ROOT/docs/finished_plans/plan-$PLAN_NAME.md"
SISYPHUS_PLAN="$REPO_ROOT/.sisyphus/plans/$PLAN_NAME.md"

# ──────────────────────────────────────────────────────────────────
# 幂等短路：已归档就直接退出
# ──────────────────────────────────────────────────────────────────

if [[ -f "$PLAN_ARCHIVED" && ! -f "$PLAN_ACTIVE" ]]; then
  echo "[info] plan-$PLAN_NAME 已归档在 docs/finished_plans/，跳过"
  exit 0
fi

# ──────────────────────────────────────────────────────────────────
# 基础校验
# ──────────────────────────────────────────────────────────────────

if [[ ! -f "$PLAN_ACTIVE" ]]; then
  echo "错误: 活跃 plan 不存在: $PLAN_ACTIVE" >&2
  echo "提示: 若已归档请确认 $PLAN_ARCHIVED；若不存在请确认 plan 名拼写" >&2
  exit 66
fi

if [[ -f "$SISYPHUS_PLAN" ]] && grep -q "\[BLOCKED:" "$SISYPHUS_PLAN"; then
  echo "错误: .sisyphus/plans/$PLAN_NAME.md 含 [BLOCKED: ...] 标记，拒绝归档" >&2
  echo "提示: 处理完所有 BLOCKED 条目再重跑归档" >&2
  exit 2
fi

# ──────────────────────────────────────────────────────────────────
# 归档
# ──────────────────────────────────────────────────────────────────

mkdir -p "$REPO_ROOT/docs/finished_plans"
git -C "$REPO_ROOT" mv "docs/plan-$PLAN_NAME.md" "docs/finished_plans/plan-$PLAN_NAME.md"

# ──────────────────────────────────────────────────────────────────
# Commit（git-master 的风格若不可用则用朴素中文）
# ──────────────────────────────────────────────────────────────────

TITLE="$(head -n 1 "$PLAN_ARCHIVED" | sed -E 's/^#+[[:space:]]*//')"
COMMIT_MSG="归档 plan-$PLAN_NAME"
if [[ -n "$TITLE" ]]; then
  COMMIT_MSG="$COMMIT_MSG：$TITLE"
fi

git -C "$REPO_ROOT" commit -m "$COMMIT_MSG"
echo "[ok] 归档完成：$COMMIT_MSG"
