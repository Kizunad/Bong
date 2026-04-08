#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/library-common.sh"

write_main_index() {
  local index_file="$LIBRARY_ROOT/index.md"
  local tmp
  local slug

  tmp="$(mktemp)"

  {
    cat <<'EOF'
# 末法残土图书馆总目

> “骨币会贬，记载不朽。凡可立卷者，皆应先入馆藏，再论买卖与实现。”

---

## 使用方式

1. 以 `docs/library/templates/馆藏条目模板.md` 为底本，写入对应分馆。
2. 新条目写完后，运行：`bash scripts/catalog-book.sh "docs/library/<分馆>/<书名>.md"`。
3. 某个实现项落地后，运行：`bash scripts/mark-implemented.sh "docs/library/<分馆>/<书名>.md" "<实现项>"`。
4. 如果你批量整理过馆藏，或索引看起来不对，再运行：`bash scripts/rebuild-library-index.sh`。

---

## 分馆总览

> 本页由 `scripts/rebuild-library-index.sh` 自动生成。

| 分馆 | 收录册数 | 落地进度 | 索引 |
|---|---|---|---|
EOF

    for slug in "${LIBRARY_CATEGORIES[@]}"; do
      local category_dir="$LIBRARY_ROOT/$slug"
      local book_count=0
      local done_sum=0
      local total_sum=0
      local book

      while IFS= read -r -d '' book; do
        local done_count
        local total_count
        book_count=$((book_count + 1))
        read -r done_count total_count <<< "$(count_implementation "$book")"
        done_sum=$((done_sum + done_count))
        total_sum=$((total_sum + total_count))
      done < <(find "$category_dir" -type f -name '*.md' ! -name 'index.md' -print0 | sort -z)

      local progress="—"
      if [[ "$total_sum" -gt 0 ]]; then
        progress="${done_sum}/${total_sum}"
      fi

      printf '| %s | %d | %s | [进入](./%s/index.md) |\n' \
        "$(category_display_name "$slug")" \
        "$book_count" \
        "$progress" \
        "$slug"
    done
  } > "$tmp"

  mv "$tmp" "$index_file"
}

write_category_index() {
  local slug="$1"
  local category_dir="$LIBRARY_ROOT/$slug"
  local index_file="$category_dir/index.md"
  local tmp
  local book_found=0
  local book

  mkdir -p "$category_dir"
  tmp="$(mktemp)"

  {
    printf '%s\n' "# $(category_display_name "$slug")"
    printf '\n'
    printf '%s\n' "> “$(category_quote "$slug")”"
    printf '\n'
    printf '%s\n' '---'
    printf '\n'
    printf '%s\n' '## 分馆说明'
    printf '\n'
    printf '%s\n' "$(category_scope "$slug")"
    printf '\n'
    printf '%s\n' '---'
    printf '\n'
    printf '%s\n' '## 馆藏目录'
    printf '\n'
    printf '%s\n' '> 本页由 `scripts/rebuild-library-index.sh` 自动生成。'
    printf '\n'
    printf '%s\n' '| 藏书编号 | 书名 | 书架 | 估值 | 稀有度 | 实现进度 | 最后整理 |'
    printf '%s\n' '|---|---|---|---|---|---|---|'

    while IFS= read -r -d '' book; do
      local relative_path
      local title
      local shelf
      local book_id
      local value
      local rarity
      local progress
      local updated_at

      book_found=1
      relative_path="${book#$category_dir/}"
      title="$(book_title "$book")"
      shelf="$(meta_value "$book" "书架")"
      book_id="$(meta_value "$book" "藏书编号")"
      value="$(meta_value "$book" "估值")"
      rarity="$(meta_value "$book" "稀有度")"
      progress="$(implementation_progress "$book")"
      updated_at="$(meta_value "$book" "最后整理")"

      printf '| %s | [%s](./%s) | %s | %s | %s | %s | %s |\n' \
        "$book_id" \
        "$title" \
        "$relative_path" \
        "$shelf" \
        "$value" \
        "$rarity" \
        "$progress" \
        "$updated_at"
    done < <(find "$category_dir" -type f -name '*.md' ! -name 'index.md' -print0 | sort -z)

    if [[ "$book_found" -eq 0 ]]; then
      echo "| — | 暂无馆藏 | — | — | — | — | — |"
    fi
  } > "$tmp"

  mv "$tmp" "$index_file"
}

if [[ $# -gt 1 ]]; then
  echo "用法：bash scripts/rebuild-library-index.sh [world|geography|peoples|ecology|cultivation]" >&2
  exit 1
fi

if [[ $# -eq 1 && -n "$1" ]]; then
  TARGET_SLUG="$(resolve_category_slug "$1")"
  write_category_index "$TARGET_SLUG"
  write_main_index
  echo "✓ 已重建分馆索引：$TARGET_SLUG"
  echo "✓ 已同步总目：docs/library/index.md"
  exit 0
fi

for category in "${LIBRARY_CATEGORIES[@]}"; do
  write_category_index "$category"
done

write_main_index

echo "✓ 已重建全馆总目与全部分馆索引"
