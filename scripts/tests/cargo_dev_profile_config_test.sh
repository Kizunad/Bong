#!/usr/bin/env bash
# 核验 server/.cargo/config.toml 经仓库 canonical 构建令牌入口生效。
# 不改依赖版本；只证明 profile.dev.debug = line-tables-only 被 cargo 读入。
set -euo pipefail

ROOT=$(realpath "$(dirname "$0")/../..")
SERVER="$ROOT/server"
CFG="$SERVER/.cargo/config.toml"
PASS=0
FAIL=0
check() {
  local desc="$1"; shift
  if "$@"; then
    echo "  PASS: $desc"; PASS=$((PASS + 1))
  else
    echo "  FAIL: $desc"; FAIL=$((FAIL + 1))
  fi
}

echo "== 1. 配置文件存在且声明 line-tables-only"
check "config.toml 存在" test -f "$CFG"
check "含 [profile.dev]" grep -q '^\[profile\.dev\]' "$CFG"
check "含 debug = line-tables-only" grep -Eq 'debug\s*=\s*"line-tables-only"' "$CFG"

echo "== 2. 最小 crate 复现：crate 本地 .cargo/config.toml 经构建令牌 wrapper 注入 rustc"
PROBE=$(mktemp -d /tmp/cargo-profile-probe.XXXXXX)
cleanup() { rm -rf "$PROBE"; }
trap cleanup EXIT
mkdir -p "$PROBE"
(
  cd "$PROBE"
  "$ROOT/scripts/build-token.sh" cargo new --bin probe -q --name cargo_profile_probe
)
mkdir -p "$PROBE/probe/.cargo"
# 与仓库一致的 profile 片段
cp "$CFG" "$PROBE/probe/.cargo/config.toml"
(
  cd "$PROBE/probe"
  # 清可能覆盖的环境
  unset CARGO_PROFILE_DEV_DEBUG || true
  "$ROOT/scripts/build-token.sh" cargo build -v >"$PROBE/build.out" 2>&1
)
check "build 日志含 debuginfo=line-tables-only" \
  grep -q 'debuginfo=line-tables-only' "$PROBE/build.out"
check_not_false_debug() {
  # 不应出现 -C debuginfo=2（完整 debuginfo）作为主 rustc 行
  if grep -E 'bin/rustc .* -C debuginfo=2' "$PROBE/build.out" >/dev/null; then
    return 1
  fi
  return 0
}
check "主 rustc 未使用 debuginfo=2" check_not_false_debug

echo "== 3. 仓库 server 入口：构建令牌 wrapper cargo metadata 可运行（config 不破坏 canonical 入口）"
(
  cd "$SERVER"
  "$ROOT/scripts/build-token.sh" cargo metadata --no-deps --format-version 1 >"$PROBE/meta.json"
)
check "server cargo metadata 成功" test -s "$PROBE/meta.json"
# metadata 含 workspace_root 指向 server
ws=$(python3 -c 'import json,sys,os; d=json.load(open(sys.argv[1])); print(os.path.realpath(d["workspace_root"]))' "$PROBE/meta.json")
check "workspace_root 指向 server" bash -c '[[ "$1" == "$2" ]]' _ "$ws" "$(realpath "$SERVER")"

# 额外：从 server 目录解析 config 文件本身仍在 expected 路径（canonical 入口旁路）
check "server/.cargo/config.toml 相对 server 可读" test -r "$SERVER/.cargo/config.toml"

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "cargo_dev_profile_config 契约测试全部通过"
