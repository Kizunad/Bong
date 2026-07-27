#!/usr/bin/env bash
# 本机当前用户共享构建令牌：cargo 最多并发 2，gradle 最多并发 1。
#
# 用法：
#   scripts/build-token.sh cargo test --locked
#   scripts/build-token.sh gradle test build
#
# 生产锁域与容量固定，所有 worktree 共享同一组槽位。仅本脚本的契约测试可用
# BONG_BUILD_TOKEN_TEST_MODE=1 + BONG_BUILD_TOKEN_DIR 指向私有 sandbox。
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

if [[ -v BONG_BUILD_TOKEN_CARGO_SLOTS || -v BONG_BUILD_TOKEN_GRADLE_SLOTS ]]; then
  printf '[build-token] 生产槽位固定为 cargo=2、gradle=1，不接受环境变量改写容量\n' >&2
  exit 2
fi

if [[ ${BONG_BUILD_TOKEN_TEST_MODE:-0} == 1 ]]; then
  if [[ -z ${BONG_BUILD_TOKEN_DIR:-} ]]; then
    printf '[build-token] test mode 必须显式提供 BONG_BUILD_TOKEN_DIR\n' >&2
    exit 2
  fi
  lock_root=$BONG_BUILD_TOKEN_DIR
else
  if [[ -v BONG_BUILD_TOKEN_DIR ]]; then
    printf '[build-token] 生产锁域固定；BONG_BUILD_TOKEN_DIR 仅允许配合 test mode 使用\n' >&2
    exit 2
  fi
  lock_root="/tmp/bong-build-token-v2-$(id -u)"
fi

case "$kind" in
  cargo)
    slots=2
    command=(cargo "$@")
    ;;
  gradle)
    slots=1
    command=(./gradlew "$@")
    ;;
  *)
    printf '[build-token] 不支持的构建器 %q；仅接受 cargo 或 gradle\n' "$kind" >&2
    usage
    ;;
esac

# 在 root-owned sticky /tmp 下建立当前用户私有锁域。目录一旦存在就严格核验：
# 不跟随 symlink，不接管其他 owner，不容忍宽权限或 chmod 失败。
umask 077
if [[ -L $lock_root ]]; then
  printf '[build-token] 锁目录不得是符号链接：%s\n' "$lock_root" >&2
  exit 2
fi
if ! mkdir "$lock_root" 2>/dev/null && [[ ! -d $lock_root ]]; then
  printf '[build-token] 无法安全创建锁目录：%s\n' "$lock_root" >&2
  exit 2
fi
if [[ -L $lock_root || ! -d $lock_root ]]; then
  printf '[build-token] 锁路径不是可信目录：%s\n' "$lock_root" >&2
  exit 2
fi

expected_uid=$(id -u)
read -r root_uid root_mode < <(stat -Lc '%u %a' -- "$lock_root")
if [[ $root_uid != "$expected_uid" || $root_mode != 700 ]]; then
  printf '[build-token] 锁目录必须由当前用户持有且权限为 700，实际 uid=%s mode=%s：%s\n' \
    "$root_uid" "$root_mode" "$lock_root" >&2
  exit 2
fi

prepare_lock_file() {
  local lock_file=$1
  if [[ -L $lock_file ]]; then
    printf '[build-token] 槽位锁不得是符号链接：%s\n' "$lock_file" >&2
    return 2
  fi
  if [[ ! -e $lock_file ]]; then
    (set -o noclobber; : >"$lock_file") 2>/dev/null || true
  fi
  if [[ -L $lock_file || ! -f $lock_file ]]; then
    printf '[build-token] 槽位锁不是普通文件：%s\n' "$lock_file" >&2
    return 2
  fi
  chmod 600 -- "$lock_file"
  local file_uid file_mode file_links
  read -r file_uid file_mode file_links < <(stat -Lc '%u %a %h' -- "$lock_file")
  if [[ $file_uid != "$expected_uid" || $file_mode != 600 || $file_links != 1 ]]; then
    printf '[build-token] 槽位锁必须是当前用户持有的 600 单链接普通文件，实际 uid=%s mode=%s links=%s：%s\n' \
      "$file_uid" "$file_mode" "$file_links" "$lock_file" >&2
    return 2
  fi
}

for ((slot = 1; slot <= slots; slot++)); do
  prepare_lock_file "$lock_root/$kind-$slot.lock"
done

start_seconds=$SECONDS
announced=0
attempt=0
while true; do
  for ((slot = 1; slot <= slots; slot++)); do
    lock_file="$lock_root/$kind-$slot.lock"
    attempt=$((attempt + 1))
    acquired_marker="$lock_root/.acquired-$kind-$$-$BASHPID-$attempt"

    # flock 自己打开并持有 path，--close 在 exec 构建命令前关闭该继承 FD；
    # wrapper 父进程等待 command，cargo/Gradle daemon 与残留子孙不会继承锁。
    set +e
    flock --nonblock --close --conflict-exit-code 75 "$lock_file" \
      bash -c '
        set -euo pipefail
        marker=$1
        kind=$2
        slot=$3
        slots=$4
        waited=$5
        shift 5
        : >"$marker"
        printf "[build-token] %s 获得槽位 %s/%s（等待 %ss）：" \
          "$kind" "$slot" "$slots" "$waited" >&2
        printf " %q" "$@" >&2
        printf "\n" >&2
        exec "$@"
      ' build-token-locked "$acquired_marker" "$kind" "$slot" "$slots" \
      "$((SECONDS - start_seconds))" "${command[@]}"
    status=$?
    set -e

    if [[ -e $acquired_marker ]]; then
      rm -f -- "$acquired_marker"
      if ((status == 75)); then
        printf '[build-token] 构建命令返回 75\n' >&2
      fi
      exit "$status"
    fi
    if ((status != 75)); then
      printf '[build-token] flock 槽位 %d 失败，退出码 %d\n' "$slot" "$status" >&2
      exit "$status"
    fi
  done

  if ((announced == 0)); then
    printf '[build-token] %s %d 个槽位均在使用，等待可用令牌…\n' \
      "$kind" "$slots" >&2
    announced=1
  fi
  sleep 0.2
done
