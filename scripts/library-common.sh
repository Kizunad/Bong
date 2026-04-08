#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TODAY="$(date +%F)"
LIBRARY_ROOT="$ROOT/docs/library"

LIBRARY_CATEGORIES=(world geography peoples ecology cultivation)

die() {
  echo "错误: $*" >&2
  exit 1
}

to_abs_path() {
  local input="$1"
  if [[ "$input" = /* ]]; then
    printf '%s\n' "$input"
    return
  fi

  local dir
  local base
  dir="$(dirname "$input")"
  base="$(basename "$input")"
  printf '%s/%s\n' "$(cd "$dir" && pwd)" "$base"
}

category_display_name() {
  case "$1" in
    world) echo "世界总志" ;;
    geography) echo "地理志" ;;
    peoples) echo "众生谱" ;;
    ecology) echo "生态录" ;;
    cultivation) echo "修行藏" ;;
    *) return 1 ;;
  esac
}

category_slug_from_display() {
  case "$1" in
    世界总志) echo "world" ;;
    地理志) echo "geography" ;;
    众生谱) echo "peoples" ;;
    生态录) echo "ecology" ;;
    修行藏) echo "cultivation" ;;
    *) return 1 ;;
  esac
}

category_quote() {
  case "$1" in
    world) echo "天道不语，藏书代言。" ;;
    geography) echo "路比机缘长，地比尸骨诚实。" ;;
    peoples) echo "众生各有账本，礼数只是账本外面那层纸。" ;;
    ecology) echo "草木鱼虫皆在吃灵气，只是吃法不同。" ;;
    cultivation) echo "术法再高，也逃不过真元要算账。" ;;
    *) return 1 ;;
  esac
}

category_scope() {
  case "$1" in
    world)
      echo "收录世界法则、时代演变、天道机制、历史遗绪与宏观环境补卷。"
      ;;
    geography)
      echo "收录区域、地貌、陆海、遗迹、灵脉走向与荒野路径相关条目。"
      ;;
    peoples)
      echo "收录种族、部族、宗门、势力、礼俗、交易人群与社会组织条目。"
      ;;
    ecology)
      echo "收录生物、植物、药材、食材、风味指南与生态链相关条目。"
      ;;
    cultivation)
      echo "收录流派、功法、术法、器物、丹方与修行法门相关条目。"
      ;;
    *) return 1 ;;
  esac
}

require_known_category_slug() {
  local slug="$1"
  local category
  for category in "${LIBRARY_CATEGORIES[@]}"; do
    if [[ "$category" == "$slug" ]]; then
      return 0
    fi
  done

  die "未知分馆 slug：$slug"
}

resolve_category_slug() {
  local arg="$1"

  if [[ -z "$arg" ]]; then
    die "分馆参数不能为空"
  fi

  if category_slug_from_display "$arg" >/dev/null 2>&1; then
    category_slug_from_display "$arg"
    return
  fi

  require_known_category_slug "$arg"
  echo "$arg"
}

meta_value() {
  local file="$1"
  local key="$2"

  awk -v prefix="- ${key}：" '
    index($0, prefix) == 1 {
      print substr($0, length(prefix) + 1)
      exit
    }
  ' "$file"
}

book_title() {
  local file="$1"

  awk '
    /^# / {
      sub(/^# /, "")
      print
      exit
    }
  ' "$file"
}

update_meta_line() {
  local file="$1"
  local key="$2"
  local value="$3"
  local prefix="- ${key}："
  local tmp

  tmp="$(mktemp)"

  awk -v prefix="$prefix" -v value="$value" '
    BEGIN { updated = 0 }
    index($0, prefix) == 1 {
      print prefix value
      updated = 1
      next
    }
    { print }
    END {
      if (updated == 0) {
        exit 2
      }
    }
  ' "$file" > "$tmp" || {
    rm -f "$tmp"
    die "未找到元信息字段：$key ($file)"
  }

  mv "$tmp" "$file"
}

count_implementation() {
  local file="$1"

  awk '
    BEGIN { in_section = 0; done_count = 0; total = 0 }
    /^## 实现挂钩$/ { in_section = 1; next }
    in_section && /^---$/ { in_section = 0 }
    in_section && /^- \[x\] / { done_count++; total++; next }
    in_section && /^- \[ \] / { total++; next }
    END { printf "%d %d\n", done_count, total }
  ' "$file"
}

implementation_progress() {
  local file="$1"
  local done_count
  local total

  read -r done_count total <<< "$(count_implementation "$file")"
  if [[ "$total" -eq 0 ]]; then
    echo "—"
  else
    echo "${done_count}/${total}"
  fi
}

relative_to_root() {
  local absolute_path="$1"
  local prefix="$ROOT/"

  if [[ "$absolute_path" == "$prefix"* ]]; then
    echo "${absolute_path#$prefix}"
    return
  fi

  echo "$absolute_path"
}

require_heading() {
  local file="$1"
  local heading="$2"

  grep -q "^## ${heading}$" "$file" || die "$file 缺少章节：## ${heading}"
}

validate_book_file() {
  local file="$1"

  [[ -f "$file" ]] || die "文件不存在：$file"
  [[ "$file" == *.md ]] || die "只支持 Markdown 条目：$file"

  local base
  base="$(basename "$file")"
  [[ "$base" != "index.md" ]] || die "index.md 不是可收录条目：$file"
  [[ "$file" != "$LIBRARY_ROOT/templates/"* ]] || die "模板目录下的文件不可收录：$file"
  [[ "$file" == "$LIBRARY_ROOT/"* ]] || die "条目必须位于 docs/library/ 下：$file"

  local title
  title="$(book_title "$file")"
  [[ -n "$title" ]] || die "$file 缺少书名标题"

  require_heading "$file" "编目信息"
  require_heading "$file" "摘要"
  require_heading "$file" "正文"
  require_heading "$file" "实现挂钩"

  local key
  for key in 分馆 书架 藏书编号 估值 稀有度 收录状态 锚点来源 收录时间 最后整理; do
    [[ -n "$(meta_value "$file" "$key")" ]] || die "$file 缺少元信息字段：$key"
  done

  local display_name
  local slug
  display_name="$(meta_value "$file" "分馆")"
  slug="$(category_slug_from_display "$display_name" 2>/dev/null)" || die "$file 的分馆字段无效：$display_name"

  [[ "$file" == "$LIBRARY_ROOT/$slug/"* ]] || die "$file 的路径与分馆字段不一致：应位于 docs/library/$slug/"
}
