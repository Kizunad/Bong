#!/usr/bin/env bash
# 本机共享构建令牌：cargo 最多并发 2，gradle 最多并发 1。
#
# 用法：
#   scripts/build-token.sh cargo test --locked
#   scripts/build-token.sh gradle test build
#
# 所有 worktree 共享 /tmp 下的固定槽位锁；进程退出（包括信号/崩溃）时 flock
# 随文件描述符自动释放，不维护会过期的计数文件。
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
用法：scripts/build-token.sh <cargo|gradle> <args...>
示例：scripts/build-token.sh cargo test --locked
      scripts/build-token.sh gradle test build
EOF
  exit 2
}

[[ $# -ge 1 ]] || usage
kind=$1
shift

lock_root=${BONG_BUILD_TOKEN_DIR:-/tmp/bong-build-token-v1}
case "$kind" in
  cargo)
    slots=${BONG_BUILD_TOKEN_CARGO_SLOTS:-2}
    command=(cargo "$@")
    ;;
  gradle)
    slots=${BONG_BUILD_TOKEN_GRADLE_SLOTS:-1}
    command=(./gradlew "$@")
    ;;
  *)
    printf '[build-token] 不支持的构建器 %q；仅接受 cargo 或 gradle\n' "$kind" >&2
    usage
    ;;
esac

if [[ ! "$slots" =~ ^[1-9][0-9]*$ ]]; then
  printf '[build-token] %s 槽位数必须是正整数，实际为 %q\n' "$kind" "$slots" >&2
  exit 2
fi

mkdir -p "$lock_root"
chmod 1777 "$lock_root" 2>/dev/null || true

start_seconds=$SECONDS
announced=0
while true; do
  for ((slot = 1; slot <= slots; slot++)); do
    lock_file="$lock_root/$kind-$slot.lock"
    exec {lock_fd}>"$lock_file"
    if flock -n "$lock_fd"; then
      waited=$((SECONDS - start_seconds))
      printf '[build-token] %s 获得槽位 %d/%d（等待 %ds）：' \
        "$kind" "$slot" "$slots" "$waited" >&2
      printf ' %q' "${command[@]}" >&2
      printf '\n' >&2
      exec "${command[@]}"
    fi
    exec {lock_fd}>&-
  done

  if ((announced == 0)); then
    printf '[build-token] %s %d 个槽位均在使用，等待可用令牌…\n' \
      "$kind" "$slots" >&2
    announced=1
  fi
  sleep 0.2
done
