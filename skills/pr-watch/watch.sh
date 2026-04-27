#!/usr/bin/env bash
# pr-watch: block until a PR gets a new comment/review, or timeout.
#
# Polls GitHub for issue comments, PR review comments, and submitted reviews.
# Compares against a baseline snapshot taken at start; exits on first diff.
#
# Exit codes:
#   0   new activity detected (details printed to stdout)
#   10  timeout reached with no new activity
#   2   usage / env error

set -euo pipefail

PR=""
TIMEOUT_SEC=540   # 9 min default (stay under Claude Code Bash 10-min ceiling)
INTERVAL_SEC=60
REPO=""

usage() {
  cat >&2 <<'EOF'
Usage: watch.sh <PR_NUM> [--timeout SEC] [--interval SEC] [--repo OWNER/REPO]

  PR_NUM       PR number to watch
  --timeout    total seconds to watch (default 540, max 540)
  --interval   seconds between polls (default 60)
  --repo       OWNER/REPO override (default: auto-detect from cwd)

Exit: 0=new activity, 10=timeout, 2=usage error
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout)  TIMEOUT_SEC="$2"; shift 2 ;;
    --interval) INTERVAL_SEC="$2"; shift 2 ;;
    --repo)     REPO="$2"; shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    -*) echo "unknown flag: $1" >&2; usage; exit 2 ;;
    *)  PR="$1"; shift ;;
  esac
done

[[ -z "$PR" ]] && { usage; exit 2; }

if [[ -z "$REPO" ]]; then
  REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null) || {
    echo "error: no --repo and auto-detect failed" >&2
    exit 2
  }
fi

# Cap to Bash ceiling with buffer
if (( TIMEOUT_SEC > 540 )); then
  echo "[warn] timeout capped at 540s (Bash 10-min ceiling); reissue for longer watch" >&2
  TIMEOUT_SEC=540
fi

snapshot() {
  # Tag each item with kind|id|timestamp so we detect both new posts and edits
  {
    gh api --paginate "repos/$REPO/issues/$PR/comments" \
      --jq '.[] | "issue-comment|\(.id)|\(.updated_at)"' 2>/dev/null || true
    gh api --paginate "repos/$REPO/pulls/$PR/comments" \
      --jq '.[] | "review-comment|\(.id)|\(.updated_at)"' 2>/dev/null || true
    gh api --paginate "repos/$REPO/pulls/$PR/reviews" \
      --jq '.[] | select(.state != "PENDING") | "review|\(.id)|\(.submitted_at)"' 2>/dev/null || true
  } | sort
}

BASELINE=$(mktemp)
CURRENT=""
cleanup() {
  [[ -n "$BASELINE" ]] && rm -f "$BASELINE"
  [[ -n "$CURRENT"  ]] && rm -f "$CURRENT"
  return 0
}
trap cleanup EXIT

snapshot > "$BASELINE"
BASELINE_COUNT=$(wc -l < "$BASELINE" | tr -d ' ')

printf '[watch] %s#%s — baseline %s items, timeout %ss, interval %ss\n' \
  "$REPO" "$PR" "$BASELINE_COUNT" "$TIMEOUT_SEC" "$INTERVAL_SEC"

DEADLINE=$(( $(date +%s) + TIMEOUT_SEC ))
POLLS=0

while (( $(date +%s) < DEADLINE )); do
  REMAIN=$(( DEADLINE - $(date +%s) ))
  SLEEP=$(( INTERVAL_SEC < REMAIN ? INTERVAL_SEC : REMAIN ))
  (( SLEEP > 0 )) && sleep "$SLEEP"

  POLLS=$(( POLLS + 1 ))
  CURRENT=$(mktemp)
  snapshot > "$CURRENT"

  NEW=$(comm -13 "$BASELINE" "$CURRENT" || true)

  if [[ -n "$NEW" ]]; then
    printf '[NEW] activity detected after %s polls\n\n' "$POLLS"
    while IFS='|' read -r kind id ts; do
      [[ -z "$kind" ]] && continue
      case "$kind" in
        issue-comment)
          gh api "repos/$REPO/issues/comments/$id" 2>/dev/null | jq -r '
            "=== issue-comment ===",
            "user: \(.user.login)",
            "at:   \(.created_at)",
            "url:  \(.html_url)",
            "---",
            (.body // "(empty)"),
            ""
          '
          ;;
        review-comment)
          gh api "repos/$REPO/pulls/comments/$id" 2>/dev/null | jq -r '
            "=== review-comment ===",
            "user: \(.user.login)",
            "at:   \(.created_at)",
            "file: \(.path):\(.line // .original_line // "?")",
            "url:  \(.html_url)",
            "---",
            (.body // "(empty)"),
            ""
          '
          ;;
        review)
          gh api "repos/$REPO/pulls/$PR/reviews/$id" 2>/dev/null | jq -r '
            "=== review (\(.state)) ===",
            "user: \(.user.login)",
            "at:   \(.submitted_at)",
            "url:  \(.html_url)",
            "---",
            (.body // "(no body)"),
            ""
          '
          ;;
      esac
    done <<< "$NEW"
    exit 0
  fi

  rm -f "$CURRENT"
  CURRENT=""
done

printf '[TIMEOUT] no new activity in %ss (%s polls)\n' "$TIMEOUT_SEC" "$POLLS"
exit 10
