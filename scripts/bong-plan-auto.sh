#!/usr/bin/env bash
# scripts/bong-plan-auto.sh
#
# Bong plan 全自动消费入口：
#   docs/plan-<name>.md  ──>  git worktree ──>  opencode + oh-my-opencode
#                                                 │
#                                                 ▼
#              Prometheus → Metis → Momus → Atlas → 归档 → push
#
# 用法：
#   bash scripts/bong-plan-auto.sh <plan-name>
#   例：bash scripts/bong-plan-auto.sh HUD-v1
#
# 名字不含 "plan-" 前缀和 ".md" 后缀；脚本会自动找 docs/plan-<name>.md。
#
# 依赖：
#   - opencode CLI（https://opencode.ai）
#   - oh-my-opencode 插件（opencode.json 已声明）
#   - OPENAI_API_KEY 或其它 gpt-5.4 provider 凭据
#
# 运行态产出：
#   .worktrees/plan-<name>/                    （独立 worktree，gitignored）
#   .worktrees/plan-<name>/.sisyphus/plans/…   （Prometheus 规整结果）
#   .worktrees/plan-<name>/.sisyphus/boulder.json （Atlas 状态，支持中断恢复）
#   分支 auto/plan-<name>                       （成功后 push origin）
#
# 退出码：
#   0 = Atlas 完成（<promise>DONE</promise>）并已 push
#   2 = BLOCKED（部分 TODO 失败，worktree 保留待人工介入）
#   其它 = 基础设施错误（worktree/opencode/push）

set -euo pipefail

# ──────────────────────────────────────────────────────────────────
# 参数解析
# ──────────────────────────────────────────────────────────────────

if [[ $# -ne 1 ]]; then
  echo "用法: $0 <plan-name>" >&2
  echo "  例: $0 HUD-v1     # 消费 docs/plan-HUD-v1.md" >&2
  exit 64
fi

PLAN_NAME="$1"
REPO_ROOT="$(git rev-parse --show-toplevel)"
PLAN_SRC="$REPO_ROOT/docs/plan-$PLAN_NAME.md"
WORKTREE_DIR="$REPO_ROOT/.worktrees/plan-$PLAN_NAME"
BRANCH="auto/plan-$PLAN_NAME"
BASE_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD)"
PROMPT_TEMPLATE="$REPO_ROOT/.opencode/prompts/auto-consume.md"

# ──────────────────────────────────────────────────────────────────
# 校验
# ──────────────────────────────────────────────────────────────────

if [[ ! -f "$PLAN_SRC" ]]; then
  echo "错误: plan 文件不存在: $PLAN_SRC" >&2
  echo "提示: 检查 docs/plans-skeleton/ 或 docs/finished_plans/ —— 这两处不可消费" >&2
  exit 66
fi

if [[ "$PLAN_SRC" == *"/plans-skeleton/"* || "$PLAN_SRC" == *"/finished_plans/"* ]]; then
  echo "错误: 拒绝消费骨架或已归档 plan" >&2
  exit 66
fi

if ! command -v opencode >/dev/null 2>&1; then
  echo "错误: 未找到 opencode CLI。安装参考：https://opencode.ai" >&2
  exit 69
fi

if [[ ! -f "$PROMPT_TEMPLATE" ]]; then
  echo "错误: 缺少 prompt 模板: $PROMPT_TEMPLATE" >&2
  exit 70
fi

# ──────────────────────────────────────────────────────────────────
# Worktree 准备（幂等）
# ──────────────────────────────────────────────────────────────────

mkdir -p "$REPO_ROOT/.worktrees"

if [[ -d "$WORKTREE_DIR" ]]; then
  echo "[info] worktree 已存在，复用续跑：$WORKTREE_DIR"
elif git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$BRANCH"; then
  echo "[info] 分支 $BRANCH 已存在，checkout 到 worktree"
  git -C "$REPO_ROOT" worktree add "$WORKTREE_DIR" "$BRANCH"
else
  echo "[info] 创建 worktree + 新分支 $BRANCH（基于 $BASE_BRANCH）"
  git -C "$REPO_ROOT" worktree add -b "$BRANCH" "$WORKTREE_DIR" "$BASE_BRANCH"
fi

# ──────────────────────────────────────────────────────────────────
# 将 plan 拷入 worktree 的 .sisyphus/inputs/
# ──────────────────────────────────────────────────────────────────

mkdir -p "$WORKTREE_DIR/.sisyphus/inputs" "$WORKTREE_DIR/.sisyphus/plans"
cp "$PLAN_SRC" "$WORKTREE_DIR/.sisyphus/inputs/$PLAN_NAME.md"
echo "[info] plan 快照 → .sisyphus/inputs/$PLAN_NAME.md"

# ──────────────────────────────────────────────────────────────────
# 渲染启动 prompt（替换 {{PLAN_NAME}}）
# ──────────────────────────────────────────────────────────────────

RENDERED_PROMPT="$WORKTREE_DIR/.sisyphus/auto-consume.rendered.md"
sed "s|{{PLAN_NAME}}|$PLAN_NAME|g" "$PROMPT_TEMPLATE" > "$RENDERED_PROMPT"

# ──────────────────────────────────────────────────────────────────
# 启动 opencode（non-interactive，在 worktree 内）
# 用 /ulw-loop 包裹，omo 自己循环直到 <promise>DONE|BLOCKED</promise>
# ──────────────────────────────────────────────────────────────────

cd "$WORKTREE_DIR"

echo "[info] 启动 opencode（主模型 gpt-5.4，非交互，worktree=$WORKTREE_DIR）"
echo "[info] 流水线：Prometheus → Metis → Momus → Atlas"
echo "──────────────────────────────────────────────────────────────"

# opencode run 非交互执行；stdin 传入启动 prompt。
# 若你的 opencode 版本 CLI flag 名不同，调整这里即可（opencode --help 查）。
OPENCODE_EXIT=0
cat "$RENDERED_PROMPT" | opencode run --prompt-stdin || OPENCODE_EXIT=$?

echo "──────────────────────────────────────────────────────────────"
echo "[info] opencode 退出码: $OPENCODE_EXIT"

# ──────────────────────────────────────────────────────────────────
# 判定 DONE / BLOCKED（Atlas 的 <promise> 输出在 session transcript 里）
# 简化判据：看 docs/plan-<name>.md 是否已归档 + 有无 BLOCKED 标记
# ──────────────────────────────────────────────────────────────────

PLAN_STILL_ACTIVE="$WORKTREE_DIR/docs/plan-$PLAN_NAME.md"
PLAN_ARCHIVED="$WORKTREE_DIR/docs/finished_plans/plan-$PLAN_NAME.md"
HAS_BLOCKED=0
if grep -q "\[BLOCKED:" "$WORKTREE_DIR/.sisyphus/plans/$PLAN_NAME.md" 2>/dev/null; then
  HAS_BLOCKED=1
fi

FINAL_EXIT=0
if [[ $HAS_BLOCKED -eq 1 ]]; then
  echo "[status] BLOCKED：$WORKTREE_DIR/.sisyphus/plans/$PLAN_NAME.md 含 [BLOCKED: ...] 标注"
  echo "[status] worktree 保留，boulder.json 记录进度。修复后重跑同条命令可续。"
  FINAL_EXIT=2
elif [[ -f "$PLAN_ARCHIVED" && ! -f "$PLAN_STILL_ACTIVE" ]]; then
  echo "[status] DONE：plan 已归档到 docs/finished_plans/"
else
  echo "[status] 未检测到归档也未见 BLOCKED 标记 —— 可能 opencode 中途退出或未完成"
  FINAL_EXIT=3
fi

# ──────────────────────────────────────────────────────────────────
# Push（仅 DONE 态；BLOCKED 时 commits 留在 worktree 等人工 review）
# 失败重试 4 次指数退避（遵循仓库 git push 规约）
# ──────────────────────────────────────────────────────────────────

if [[ $FINAL_EXIT -eq 0 ]]; then
  echo "[info] push 分支 $BRANCH 到 origin"
  RETRIES=(2 4 8 16)
  PUSH_OK=0
  for i in 0 1 2 3; do
    if git -C "$WORKTREE_DIR" push -u origin "$BRANCH"; then
      PUSH_OK=1
      break
    fi
    SLEEP="${RETRIES[$i]}"
    echo "[warn] push 失败，${SLEEP}s 后重试（第 $((i+1))/4 次）"
    sleep "$SLEEP"
  done

  if [[ $PUSH_OK -eq 0 ]]; then
    echo "[error] push 重试 4 次仍失败。worktree 保留在 $WORKTREE_DIR" >&2
    FINAL_EXIT=71
  else
    echo "[ok] 已 push origin/$BRANCH"
    echo "[next] 人工 review 后开 PR（脚本不自动开）："
    echo "       worktree: $WORKTREE_DIR"
    echo "       分支:     $BRANCH"
    echo "       review 完成后清理: git worktree remove $WORKTREE_DIR"
  fi
fi

exit $FINAL_EXIT
