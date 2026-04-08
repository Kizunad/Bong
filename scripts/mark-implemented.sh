#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/library-common.sh"

usage() {
  cat <<'EOF'
用法：
  bash scripts/mark-implemented.sh "docs/library/<分馆>/<书名>.md" "<实现项>"
EOF
}

if [[ $# -ne 2 ]]; then
  usage
  exit 1
fi

BOOK_PATH="$(to_abs_path "$1")"
ITEM="$2"

validate_book_file "$BOOK_PATH"

TMP_FILE="$(mktemp)"
UPDATED=0
ALREADY_DONE=0
IN_SECTION=0

while IFS= read -r line || [[ -n "$line" ]]; do
  if [[ "$line" == "## 实现挂钩" ]]; then
    IN_SECTION=1
    printf '%s\n' "$line" >> "$TMP_FILE"
    continue
  fi

  if [[ "$IN_SECTION" -eq 1 && "$line" == "---" ]]; then
    IN_SECTION=0
  fi

  if [[ "$IN_SECTION" -eq 1 && "$UPDATED" -eq 0 ]]; then
    if [[ "$line" == "- [ ] $ITEM" ]]; then
      printf '%s\n' "- [x] $ITEM" >> "$TMP_FILE"
      UPDATED=1
      continue
    fi

    if [[ "$line" == "- [x] $ITEM" ]]; then
      printf '%s\n' "$line" >> "$TMP_FILE"
      UPDATED=1
      ALREADY_DONE=1
      continue
    fi
  fi

  printf '%s\n' "$line" >> "$TMP_FILE"
done < "$BOOK_PATH"

if [[ "$UPDATED" -eq 0 ]]; then
  rm -f "$TMP_FILE"
  die "未在“## 实现挂钩”里找到实现项：$ITEM"
fi

mv "$TMP_FILE" "$BOOK_PATH"
update_meta_line "$BOOK_PATH" "最后整理" "$TODAY"

CATEGORY_SLUG="$(category_slug_from_display "$(meta_value "$BOOK_PATH" "分馆")")"
bash "$ROOT/scripts/rebuild-library-index.sh" "$CATEGORY_SLUG"

PROGRESS="$(implementation_progress "$BOOK_PATH")"

if [[ "$ALREADY_DONE" -eq 1 ]]; then
  echo "✓ 实现项本来就已完成：$ITEM"
else
  echo "✓ 已记录实现：$ITEM"
fi

echo "  当前进度：$PROGRESS"
echo "  条目：$(relative_to_root "$BOOK_PATH")"
