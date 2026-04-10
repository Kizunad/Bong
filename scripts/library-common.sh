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

relative_to_root() {
  local absolute_path="$1"
  local prefix="$ROOT/"

  if [[ "$absolute_path" == "$prefix"* ]]; then
    echo "${absolute_path#$prefix}"
    return
  fi

  echo "$absolute_path"
}

# ── JSON 操作（需要 node） ──────────────────────────

json_field() {
  local file="$1"
  local field_path="$2"
  node -e "
    const d = require('${file}');
    const keys = '${field_path}'.split('.');
    let v = d;
    for (const k of keys) { v = v?.[k]; }
    if (v !== undefined && v !== null) process.stdout.write(String(v));
  " 2>/dev/null
}

json_set_field() {
  local file="$1"
  local field_path="$2"
  local value="$3"
  node -e "
    const fs = require('fs');
    const d = JSON.parse(fs.readFileSync('${file}', 'utf-8'));
    const keys = '${field_path}'.split('.');
    let obj = d;
    for (let i = 0; i < keys.length - 1; i++) {
      if (!obj[keys[i]]) obj[keys[i]] = {};
      obj = obj[keys[i]];
    }
    obj[keys[keys.length - 1]] = '${value}';
    fs.writeFileSync('${file}', JSON.stringify(d, null, 2) + '\n', 'utf-8');
  "
}

json_count_todos() {
  local file="$1"
  node -e "
    const d = require('${file}');
    const todos = d.implementation?.todos || [];
    const done = todos.filter(t => t.done).length;
    process.stdout.write(done + ' ' + todos.length);
  " 2>/dev/null
}

book_title() {
  local file="$1"
  json_field "$file" "title"
}

meta_value() {
  local file="$1"
  local label="$2"

  case "$label" in
    书架) json_field "$file" "catalog.shelf" ;;
    藏书编号) json_field "$file" "catalog.id" ;;
    估值) json_field "$file" "catalog.value" ;;
    稀有度) json_field "$file" "catalog.rarity" ;;
    最后整理) json_field "$file" "catalog.lastEdit" ;;
    *) die "未知元数据字段：$label" ;;
  esac
}

count_implementation() {
  json_count_todos "$1"
}

implementation_progress() {
  local file="$1"
  local done total
  read -r done total <<< "$(count_implementation "$file")"
  if [[ "$total" -eq 0 ]]; then
    echo "—"
  else
    echo "${done}/${total}"
  fi
}

validate_book_file() {
  local file="$1"

  [[ -f "$file" ]] || die "文件不存在：$file"
  [[ "$file" == *.json ]] || die "只支持 JSON 条目：$file"

  local base
  base="$(basename "$file")"
  [[ "$file" != "$LIBRARY_ROOT/templates/"* ]] || die "模板目录下的文件不可收录：$file"
  [[ "$file" == "$LIBRARY_ROOT/"* ]] || die "条目必须位于 docs/library/ 下：$file"

  # 验证 JSON 可解析且有必要字段
  node -e "
    const d = require('${file}');
    if (!d.title) { process.stderr.write('缺少 title'); process.exit(1); }
    if (!d.catalog) { process.stderr.write('缺少 catalog'); process.exit(1); }
    const required = ['hall','shelf','id','value','rarity','status','anchor','date','lastEdit'];
    for (const k of required) {
      if (!d.catalog[k] && d.catalog[k] !== '') {
        process.stderr.write('缺少 catalog.' + k);
        process.exit(1);
      }
    }
  " 2>&1 || die "$file JSON 校验失败"

  local display_name
  local slug
  display_name="$(json_field "$file" "catalog.hall")"
  slug="$(category_slug_from_display "$display_name" 2>/dev/null)" || die "$file 的分馆字段无效：$display_name"

  [[ "$file" == "$LIBRARY_ROOT/$slug/"* ]] || die "$file 的路径与分馆字段不一致：应位于 docs/library/$slug/"
}
