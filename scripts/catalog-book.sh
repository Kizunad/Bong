#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/library-common.sh"

usage() {
  cat <<'EOF'
用法：
  bash scripts/catalog-book.sh "docs/library/<分馆>/<书名>.json"
EOF
}

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

BOOK_PATH="$(to_abs_path "$1")"
validate_book_file "$BOOK_PATH"

DISPLAY_NAME="$(json_field "$BOOK_PATH" "catalog.hall")"
CATEGORY_SLUG="$(category_slug_from_display "$DISPLAY_NAME")"
TITLE="$(json_field "$BOOK_PATH" "title")"

STATUS="$(json_field "$BOOK_PATH" "catalog.status")"
COLLECTED_AT="$(json_field "$BOOK_PATH" "catalog.date")"

if [[ -z "$STATUS" || "$STATUS" == "待收录" ]]; then
  json_set_field "$BOOK_PATH" "catalog.status" "在架"
fi

if [[ -z "$COLLECTED_AT" || "$COLLECTED_AT" == "待收录" ]]; then
  json_set_field "$BOOK_PATH" "catalog.date" "$TODAY"
fi

json_set_field "$BOOK_PATH" "catalog.lastEdit" "$TODAY"

bash "$ROOT/scripts/rebuild-library-index.sh" "$CATEGORY_SLUG"

PROGRESS="$(json_count_todos "$BOOK_PATH")"
BOOK_ID="$(json_field "$BOOK_PATH" "catalog.id")"
VALUE="$(json_field "$BOOK_PATH" "catalog.value")"
read -r DONE TOTAL <<< "$PROGRESS"

if [[ "$TOTAL" -eq 0 ]]; then
  PROGRESS_STR="—"
else
  PROGRESS_STR="${DONE}/${TOTAL}"
fi

echo "✓ 已收录：《${TITLE}》"
echo "  分馆：$DISPLAY_NAME"
echo "  藏书编号：$BOOK_ID"
echo "  估值：$VALUE"
echo "  实现进度：$PROGRESS_STR"
echo "  路径：$(relative_to_root "$BOOK_PATH")"
