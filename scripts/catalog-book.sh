#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/library-common.sh"

usage() {
  cat <<'EOF'
用法：
  bash scripts/catalog-book.sh "docs/library/<分馆>/<书名>.md"
EOF
}

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

BOOK_PATH="$(to_abs_path "$1")"
validate_book_file "$BOOK_PATH"

DISPLAY_NAME="$(meta_value "$BOOK_PATH" "分馆")"
CATEGORY_SLUG="$(category_slug_from_display "$DISPLAY_NAME")"
TITLE="$(book_title "$BOOK_PATH")"

STATUS="$(meta_value "$BOOK_PATH" "收录状态")"
COLLECTED_AT="$(meta_value "$BOOK_PATH" "收录时间")"

if [[ -z "$STATUS" || "$STATUS" == "待收录" ]]; then
  update_meta_line "$BOOK_PATH" "收录状态" "在架"
fi

if [[ -z "$COLLECTED_AT" || "$COLLECTED_AT" == "待收录" ]]; then
  update_meta_line "$BOOK_PATH" "收录时间" "$TODAY"
fi

update_meta_line "$BOOK_PATH" "最后整理" "$TODAY"

bash "$ROOT/scripts/rebuild-library-index.sh" "$CATEGORY_SLUG"

PROGRESS="$(implementation_progress "$BOOK_PATH")"
BOOK_ID="$(meta_value "$BOOK_PATH" "藏书编号")"
VALUE="$(meta_value "$BOOK_PATH" "估值")"

echo "✓ 已收录：$TITLE"
echo "  分馆：$DISPLAY_NAME"
echo "  藏书编号：$BOOK_ID"
echo "  估值：$VALUE"
echo "  实现进度：$PROGRESS"
echo "  路径：$(relative_to_root "$BOOK_PATH")"
