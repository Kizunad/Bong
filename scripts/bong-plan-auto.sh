#!/usr/bin/env bash
# scripts/bong-plan-auto.sh
#
# Bong plan 全自动消费入口：
#   docs/plan-<name>.md  ──>  git worktree ──>  opencode + oh-my-opencode
#                                                 │
#                                                 ▼
#              Prometheus → Metis → Momus → Atlas → 归档 → push → PR → CI → review → merge
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
#   - bunx 或 npx（用于调 `bunx oh-my-opencode run --enforce-completion`）
#     若都不可用，会回退到 `opencode run`（无 --enforce-completion，靠 prompt 里的
#     "ultrawork high accuracy" 关键词触发 keyword-detector + ralph-loop 做持久化兜底）
#   - OPENAI_API_KEY 或其它 gpt-5.4 provider 凭据
#
# 环境变量（可选）：
#   BONG_PLAN_TIMEOUT  —— omo run 的超时秒数，默认 7200（2 小时）
#
# 运行态产出：
#   .worktree/plan-<name>/                    （独立 worktree，gitignored）
#   .worktree/plan-<name>/.sisyphus/plans/…   （Prometheus 规整结果）
#   .worktree/plan-<name>/.sisyphus/boulder.json （Atlas 状态，支持中断恢复）
#   分支 auto/plan-<name> / GitHub PR           （成功后自动开 PR 并守 CI/review）
#
# 退出码：
#   0 = plan 已 merged，远端分支已删，本地 worktree 已清理
#   2 = BLOCKED（部分 TODO 失败，worktree 保留待人工介入）
#   71 = push 失败
#   72 = PR 创建/查询失败
#   73 = CI 检查失败
#   74 = review 超时或检测到需人工处理的反馈
#   75 = merge 或收尾清理失败
#   其它 = 基础设施错误（worktree/opencode）

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
WORKTREE_DIR="$REPO_ROOT/.worktree/plan-$PLAN_NAME"
BRANCH="auto/plan-$PLAN_NAME"
BASE_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD)"
PROMPT_TEMPLATE="$REPO_ROOT/.opencode/prompts/auto-consume.md"
REVIEW_BLOCKING_RE='建议|可以考虑|推荐|最好|参考|更好的做法|不如|为什么|是否需要|担心|确定.*吗|request changes|changes requested|must fix|必须修改|有 bug|bug|不对|should|consider|question|why'
REVIEW_POSITIVE_RE='(^|[^a-z])(lgtm|approved|looks good|ship it)([^a-z]|$)|整体没问题|没问题|可以合|可合|通过|符合 claude\.md 约定'
REVIEW_SUMMARY_RE='^(#+[[:space:]]*)?(总结|摘要|主要改动|本次改动|变更摘要|实施摘要)'

fetch_pr_feedback() {
  local repo="$1"
  local pr_num="$2"
  local out_file="$3"
  local issue_file review_comment_file review_file

  issue_file=$(mktemp)
  review_comment_file=$(mktemp)
  review_file=$(mktemp)

  gh api --paginate --slurp "repos/$repo/issues/$pr_num/comments" > "$issue_file"
  gh api --paginate --slurp "repos/$repo/pulls/$pr_num/comments" > "$review_comment_file"
  gh api --paginate --slurp "repos/$repo/pulls/$pr_num/reviews" > "$review_file"

  jq -n \
    --slurpfile issue "$issue_file" \
    --slurpfile review_comments "$review_comment_file" \
    --slurpfile reviews "$review_file" \
    '{
      issue_comments: (($issue[0] // []) | add // []),
      review_comments: (($review_comments[0] // []) | add // []),
      reviews: ((($reviews[0] // []) | add // []) | map(select(.state != "PENDING")))
    }' > "$out_file"

  rm -f "$issue_file" "$review_comment_file" "$review_file"
}

feedback_is_mergeable() {
  local feedback_file="$1"
  local body

  if jq -e '.review_comments | length > 0' "$feedback_file" >/dev/null; then
    return 1
  fi

  if jq -e '.reviews[]? | select(.state == "CHANGES_REQUESTED")' "$feedback_file" >/dev/null; then
    return 1
  fi

  while IFS= read -r body; do
    [[ -z "${body//[[:space:]]/}" ]] && continue
    if printf '%s\n' "$body" | grep -Eiq "$REVIEW_BLOCKING_RE"; then
      return 1
    fi
    if printf '%s\n' "$body" | grep -Eiq "$REVIEW_POSITIVE_RE"; then
      continue
    fi
    if printf '%s\n' "$body" | grep -Eq "$REVIEW_SUMMARY_RE"; then
      continue
    fi
    return 1
  done < <(jq -r '(.issue_comments[]?.body // empty), (.reviews[]?.body // empty)' "$feedback_file")

  return 0
}

print_feedback_details() {
  local feedback_file="$1"

  jq -r '
    .issue_comments[]? |
      "=== issue-comment ===\n" +
      "user: " + (.user.login // "unknown") + "\n" +
      "at:   " + (.created_at // "?") + "\n" +
      "url:  " + (.html_url // "") + "\n" +
      "---\n" +
      (.body // "(empty)") + "\n"
  ' "$feedback_file"

  jq -r '
    .review_comments[]? |
      "=== review-comment ===\n" +
      "user: " + (.user.login // "unknown") + "\n" +
      "at:   " + (.created_at // "?") + "\n" +
      "file: " + (.path // "?") + ":" + ((.line // .original_line // "?") | tostring) + "\n" +
      "url:  " + (.html_url // "") + "\n" +
      "---\n" +
      (.body // "(empty)") + "\n"
  ' "$feedback_file"

  jq -r '
    .reviews[]? |
      "=== review (" + (.state // "UNKNOWN") + ") ===\n" +
      "user: " + (.user.login // "unknown") + "\n" +
      "at:   " + (.submitted_at // "?") + "\n" +
      "url:  " + (.html_url // "") + "\n" +
      "---\n" +
      (.body // "(no body)") + "\n"
  ' "$feedback_file"
}

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

if ! command -v gh >/dev/null 2>&1; then
  echo "错误: 未找到 gh CLI。该脚本现在会自动开 PR / 查 CI / 等 review / merge。" >&2
  exit 69
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "错误: 未找到 jq。pr-watch / review 判定需要 jq。" >&2
  exit 69
fi

if [[ ! -f "$PROMPT_TEMPLATE" ]]; then
  echo "错误: 缺少 prompt 模板: $PROMPT_TEMPLATE" >&2
  exit 70
fi

# ──────────────────────────────────────────────────────────────────
# Worktree 准备（幂等）
# ──────────────────────────────────────────────────────────────────

mkdir -p "$REPO_ROOT/.worktree"

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
#
# 首选：bunx oh-my-opencode run --enforce-completion
#   omo 的 run 包装，--enforce-completion 保证 session 活到所有 TODO 完成
#   --timeout 给兜底上限（默认 7200s = 2h，可用 BONG_PLAN_TIMEOUT 覆盖）
#
# 次选：npx oh-my-opencode run --enforce-completion（bunx 不可用时）
#
# 兜底：opencode run "<prompt>"
#   base opencode CLI；无 --enforce-completion，靠 prompt 里的 "ultrawork high accuracy"
#   关键词激活 keyword-detector + ralph-loop 做持久化。功能差一点但能跑。
#
# opencode CLI：`opencode run [message..]` —— prompt 是位置参数，不走 stdin
# ──────────────────────────────────────────────────────────────────

cd "$WORKTREE_DIR"

echo "[info] 启动 opencode（主模型 gpt-5.4，非交互，worktree=$WORKTREE_DIR）"
echo "[info] 流水线：Prometheus → Metis → Momus → Atlas"
echo "──────────────────────────────────────────────────────────────"

PROMPT_BODY="$(cat "$RENDERED_PROMPT")"
TIMEOUT_SEC="${BONG_PLAN_TIMEOUT:-7200}"
OPENCODE_EXIT=0

if command -v bunx >/dev/null 2>&1; then
  echo "[info] 使用 bunx oh-my-opencode run --enforce-completion --timeout $TIMEOUT_SEC"
  bunx oh-my-opencode run \
    --enforce-completion \
    --timeout "$TIMEOUT_SEC" \
    "$PROMPT_BODY" || OPENCODE_EXIT=$?
elif command -v npx >/dev/null 2>&1; then
  echo "[info] 使用 npx oh-my-opencode run --enforce-completion --timeout $TIMEOUT_SEC"
  npx --yes oh-my-opencode run \
    --enforce-completion \
    --timeout "$TIMEOUT_SEC" \
    "$PROMPT_BODY" || OPENCODE_EXIT=$?
else
  echo "[warn] 未找到 bunx/npx，回退到 base opencode run（无 --enforce-completion）"
  echo "[warn] 持久化依赖 prompt 里的 ultrawork 关键词 + ralph-loop hook"
  opencode run "$PROMPT_BODY" || OPENCODE_EXIT=$?
fi

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
  fi
fi

if [[ $FINAL_EXIT -eq 0 ]]; then
  REPO_SLUG=$(gh repo view --json nameWithOwner -q '.nameWithOwner')
  PR_JSON=$(gh pr list --repo "$REPO_SLUG" --head "$BRANCH" --state open --json number,url --limit 1 --jq '.[0] // empty')

  if [[ -n "$PR_JSON" ]]; then
    PR_NUM=$(jq -r '.number' <<< "$PR_JSON")
    PR_URL=$(jq -r '.url' <<< "$PR_JSON")
    echo "[info] 复用已存在 PR: $PR_URL"
  else
    PR_BODY_FILE=$(mktemp)
    cat > "$PR_BODY_FILE" <<EOF
自动消费 \`docs/plan-$PLAN_NAME.md\`。

## 实施摘要
- 在独立 worktree 内完成四阶段消费、验收、归档与 push
- 详细落地见本分支 commit 与 \`.sisyphus/plans/$PLAN_NAME.md\`

## 本地测试
- Atlas 按 plan TODO 逐项执行对应子项目验收命令，全绿后才归档

🤖 Generated by opencode /consume-plan
EOF
    if ! PR_URL=$(gh pr create --repo "$REPO_SLUG" --base "$BASE_BRANCH" --title "plan-$PLAN_NAME: 自动消费并归档" --body-file "$PR_BODY_FILE"); then
      rm -f "$PR_BODY_FILE"
      echo "[error] 创建 PR 失败。worktree 保留在 $WORKTREE_DIR，分支保留在 origin/$BRANCH" >&2
      FINAL_EXIT=72
    else
      rm -f "$PR_BODY_FILE"
      PR_NUM="${PR_URL##*/}"
      echo "[ok] 已创建 PR: $PR_URL"
    fi
  fi
fi

if [[ $FINAL_EXIT -eq 0 ]]; then
  echo "[info] 等待 CI required checks 全绿"
  if gh pr checks "$PR_NUM" --repo "$REPO_SLUG" --watch --fail-fast; then
    echo "[ok] CI 已全绿"
  else
    echo "[status] CI 未通过，保留 PR / worktree / 分支，等待人工接手"
    gh pr checks "$PR_NUM" --repo "$REPO_SLUG" || true
    while IFS= read -r run_id; do
      [[ -z "$run_id" ]] && continue
      gh run view "$run_id" --log-failed || true
    done < <(gh run list --repo "$REPO_SLUG" --branch "$BRANCH" --limit 10 --json databaseId,conclusion --jq '.[] | select(.conclusion == "failure" or .conclusion == "cancelled" or .conclusion == "timed_out") | .databaseId' 2>/dev/null || true)
    echo "[info] PR: $PR_URL"
    echo "[info] worktree: $WORKTREE_DIR"
    FINAL_EXIT=73
  fi
fi

if [[ $FINAL_EXIT -eq 0 ]]; then
  FEEDBACK_FILE=$(mktemp)
  echo "[info] 等待 review 评论（最多 9 分钟）"
  set +e
  WATCH_OUTPUT=$(bash "$REPO_ROOT/.claude/skills/pr-watch/watch.sh" "$PR_NUM" --timeout 540 --interval 60 --repo "$REPO_SLUG" 2>&1)
  WATCH_EXIT=$?
  set -e
  printf '%s\n' "$WATCH_OUTPUT"

  if ! fetch_pr_feedback "$REPO_SLUG" "$PR_NUM" "$FEEDBACK_FILE"; then
    rm -f "$FEEDBACK_FILE"
    echo "[error] 拉取 review 活动失败。PR 保留：$PR_URL" >&2
    FINAL_EXIT=74
  else
    FEEDBACK_COUNT=$(jq '(.issue_comments | length) + (.review_comments | length) + (.reviews | length)' "$FEEDBACK_FILE")

    if [[ $WATCH_EXIT -eq 10 && $FEEDBACK_COUNT -eq 0 ]]; then
      echo "[status] 9 分钟内无 review 反馈，暂不 merge"
      echo "[info] PR: $PR_URL"
      echo "[info] worktree: $WORKTREE_DIR"
      FINAL_EXIT=74
    elif feedback_is_mergeable "$FEEDBACK_FILE"; then
      echo "[ok] review gate 通过，执行 squash merge"
      if gh pr merge "$PR_NUM" --repo "$REPO_SLUG" --squash --delete-branch; then
        echo "[ok] PR 已 merged: $PR_URL"
        cd "$REPO_ROOT"
        if git worktree remove "$WORKTREE_DIR"; then
          git branch -D "$BRANCH" 2>/dev/null || true
          echo "[ok] 已清理 worktree: $WORKTREE_DIR"
        else
          echo "[error] PR 已 merged，但 worktree 清理失败：$WORKTREE_DIR" >&2
          FINAL_EXIT=75
        fi
      else
        echo "[error] merge 失败。PR / worktree / 分支保留，等待人工处理" >&2
        echo "[info] PR: $PR_URL"
        echo "[info] worktree: $WORKTREE_DIR"
        FINAL_EXIT=75
      fi
    else
      echo "[status] 检测到需人工处理的 review 反馈，暂不 merge"
      print_feedback_details "$FEEDBACK_FILE"
      echo "[info] PR: $PR_URL"
      echo "[info] worktree: $WORKTREE_DIR"
      FINAL_EXIT=74
    fi

    rm -f "$FEEDBACK_FILE"
  fi
fi

exit $FINAL_EXIT
