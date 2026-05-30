#!/usr/bin/env bash
# verify-dormant-cold-start.sh
# ============================================================================
# plan-dormant-persistence-bridge-fix-v1 · P3 冷启动端到端验收脚本
#
# 把 plan §一（复现步骤）/ §P3（验收清单）固化成可重跑的全绿/红判定。验证 P0/P1/P2
# 修复后：默认 1000 dormant 从【空 Redis】冷启动能端到端持久化、重启能恢复、长跑
# 内存平稳（无 temp key 死亡螺旋）。
#
# 四项断言（与 plan §P3 step 2-4 对齐，逐项打印 PASS/FAIL，退出码反映总结果）：
#   [1] 冷启动后 `hlen bong:npc/dormant` == DORMANT_COUNT（默认 1000）—— 写真的成功了
#   [2] `--scan 'bong:npc/dormant:tmp*'` == 0 —— 无泄漏临时 key（bug ② 已止）
#       server 日志中 `[bong][redis] reconnect attempt=` 与 `timed out replacing hash`
#       计数均 == 0 —— 出站桥没被 dormant 写饿死/刷屏（bug ① 已止）
#   [3] 保留 Redis 重启 → 日志出现 `dormant NPC snapshot(s) from Redis HASH`
#       且【无】`seeded ... dormant rogue NPC snapshots` —— 恢复成功且不 re-seed
#   [4] 长跑窗口 `info memory` used_memory 平稳 + temp key 始终 0 —— 无死亡螺旋
#
# 启动方式：复用仓库 scripts/start.sh（tmux 会话 `bong`，pane `main.1` 跑 server）。
#   start.sh 真实支持的相关 env：BONG_DORMANT_ROGUE_SEED_COUNT / BONG_ROGUE_SEED_COUNT
#   / BONG_NPC_NO_DORMANT（见 start.sh:64-66），本脚本据实导出后调 start.sh，不臆造变量。
#   start.sh 只接受 `--mock` 一个 flag（没有 --server-only/--no-agent），且每次调用都会先
#   `tmux kill-session -t bong` 再重建、从不动 Redis 数据 —— 所以「再调一次 start.sh」
#   天然等价于「保留 Redis 重启」，正是 step 3 需要的。
#
# 注意：本脚本是 shell 验收 harness，不进 cargo 测试矩阵。冷启动首跑会触发 server
#   release 构建（CARGO_TARGET_DIR 见 start.sh:18，默认 /tmp/bong-target），耗时数分钟，
#   故 step 1 的等待用「轮询 hlen 至目标」而非死等 30s，BUILD_TIMEOUT 给足余量。
# ============================================================================
set -euo pipefail

# ----------------------------------------------------------------------------
# 0. 配置（均可用环境变量覆盖）
# ----------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
START_SH="${SCRIPT_DIR}/start.sh"

TMUX_SESSION="bong"
SERVER_PANE="${TMUX_SESSION}:main.1"   # start.sh 里 pane 1 = Rust server
AGENT_PANE="${TMUX_SESSION}:main.2"    # pane 2 = 天道 agent（与 dormant 持久化无关）
SERVER_PORT=25565

# 期望规模：以 plan §P3 step 1 / start.sh 默认为准（dormant 1000 + rogue 20）
DORMANT_COUNT="${DORMANT_COUNT:-1000}"
ROGUE_COUNT="${ROGUE_COUNT:-20}"

DORMANT_KEY="bong:npc/dormant"
# P0 把 temp key 确定性化（plan P0 §109：format!("{key}:tmp")），旧实现是 :tmp:<纳秒>。
# 用宽 glob `:tmp*` 同时罩住「确定性 :tmp」与「历史 :tmp:<nonce>」，断言更严不留缝。
TMP_GLOB="${DORMANT_KEY}:tmp*"
TMP_GLOB_LITERAL="${DORMANT_KEY}:tmp:*"   # plan/任务字面写法，仅作对照打印

# 日志里要数的「故障刷屏」串（均来自 server 源码实测）：
#   redis_bridge.rs:1893  "[bong][redis] reconnect attempt=..."
#   redis_bridge.rs:1505  "timed out replacing hash {key} after ..."
PAT_RECONNECT='[bong][redis] reconnect attempt='
PAT_HASH_TIMEOUT='timed out replacing hash'
# 恢复成功串：dormant/mod.rs:403 "[bong][npc] loaded N dormant NPC snapshot(s) from Redis HASH"
PAT_RESTORE='dormant NPC snapshot(s) from Redis HASH'
# 全量 seed 串：dormant/mod.rs:711 "[bong][npc] seeded N dormant rogue NPC snapshots"
PAT_SEED='dormant rogue NPC snapshots'

# 超时/窗口（秒）
BUILD_TIMEOUT="${BUILD_TIMEOUT:-900}"     # 冷启动含 release 构建，给足
RESTART_TIMEOUT="${RESTART_TIMEOUT:-180}" # 重启复用已构建二进制，应较快
MEM_WINDOW="${MEM_WINDOW:-120}"           # 内存观察窗口；设 300 可贴 plan「5 分钟」；设 0 跳过
MEM_INTERVAL="${MEM_INTERVAL:-20}"        # 内存采样间隔

# 行为开关
#   --mock：给 start.sh 传 --mock，agent 走 mock 不调真实 LLM（默认开，避免 LLM 依赖）
#   KILL_AGENT_PANE：起栈后杀掉 agent pane，验收只关心 server+redis，去噪去依赖（默认开）
MOCK_AGENT="${MOCK_AGENT:-1}"
KILL_AGENT_PANE="${KILL_AGENT_PANE:-1}"
ASSUME_YES="${VERIFY_ASSUME_YES:-0}"      # 跳过 flushall 交互确认（CI 用）

# redis-cli 调用（允许注入 host/port）
REDIS_CLI=(redis-cli)
[[ -n "${BONG_VERIFY_REDIS_HOST:-}" ]] && REDIS_CLI+=(-h "${BONG_VERIFY_REDIS_HOST}")
[[ -n "${BONG_VERIFY_REDIS_PORT:-}" ]] && REDIS_CLI+=(-p "${BONG_VERIFY_REDIS_PORT}")

LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/verify-dormant.XXXXXX")"
LOG_COLD="${LOG_DIR}/server-cold.log"
LOG_RESTART="${LOG_DIR}/server-restart.log"

for arg in "$@"; do
  case "$arg" in
    --yes|-y) ASSUME_YES=1 ;;
    --no-mock) MOCK_AGENT=0 ;;
    --keep-agent) KILL_AGENT_PANE=0 ;;
    --no-mem) MEM_WINDOW=0 ;;
    -h|--help)
      grep -E '^# ' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      echo
      echo "用法: $0 [--yes] [--no-mock] [--keep-agent] [--no-mem]"
      echo "可调 env: DORMANT_COUNT ROGUE_COUNT BUILD_TIMEOUT RESTART_TIMEOUT MEM_WINDOW MEM_INTERVAL"
      exit 0 ;;
    *) echo "未知参数: $arg（-h 看帮助）" >&2; exit 2 ;;
  esac
done

# ----------------------------------------------------------------------------
# 1. 结果记账 + 打印
# ----------------------------------------------------------------------------
FAILURES=0
declare -a RESULT_LINES=()

c_green() { printf '\033[32m%s\033[0m' "$1"; }
c_red()   { printf '\033[31m%s\033[0m' "$1"; }
c_dim()   { printf '\033[2m%s\033[0m' "$1"; }

# record PASS|FAIL "标题" "细节（期望/实际/原因）"
record() {
  local verdict="$1" title="$2" detail="${3:-}"
  if [[ "$verdict" == "PASS" ]]; then
    printf '  [%s] %s\n' "$(c_green PASS)" "$title"
  else
    printf '  [%s] %s\n' "$(c_red FAIL)" "$title"
    FAILURES=$((FAILURES + 1))
  fi
  [[ -n "$detail" ]] && printf '         %s\n' "$(c_dim "$detail")"
  RESULT_LINES+=("${verdict}|${title}")
}

section() { printf '\n=== %s ===\n' "$1"; }
info()    { printf '  · %s\n' "$1"; }
fatal()   { printf '\n%s %s\n' "$(c_red '[FATAL]')" "$1" >&2; exit 3; }

# ----------------------------------------------------------------------------
# 2. teardown：退出时收掉 tmux 会话与 pipe（保留 Redis 数据供事后排查）
# ----------------------------------------------------------------------------
cleanup() {
  tmux pipe-pane -t "$SERVER_PANE" 2>/dev/null || true
  tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# ----------------------------------------------------------------------------
# 3. 小工具
# ----------------------------------------------------------------------------
rc() { "${REDIS_CLI[@]}" "$@"; }

redis_alive() { rc ping >/dev/null 2>&1; }

hlen_dormant() { rc hlen "$DORMANT_KEY" 2>/dev/null | tr -d '[:space:]'; }

# 统计匹配 glob 的 key 数（--scan 安全，不阻塞 Redis）
scan_count() { rc --scan --pattern "$1" 2>/dev/null | grep -c . || true; }

used_memory_bytes() {
  rc info memory 2>/dev/null | tr -d '\r' | awk -F: '/^used_memory:/{print $2; exit}'
}

# server 进程是否还占着 25565（/dev/tcp 探活，纯 bash 无需额外工具）
port_in_use() {
  (exec 3<>"/dev/tcp/127.0.0.1/${SERVER_PORT}") 2>/dev/null && { exec 3>&- 3<&-; return 0; } || return 1
}

# 把 server pane 输出导到文件（pipe-pane）。先停旧 pipe 再起，避免 toggle 关掉。
attach_server_pipe() {
  local logfile="$1" tries=0
  : >"$logfile"
  # 等 pane 出现（start.sh 同步建 pane，正常立即就绪，仍留重试余地）
  while ! tmux list-panes -t "$TMUX_SESSION" -F '#{pane_id} #{pane_index}' 2>/dev/null | grep -q ' 1$'; do
    tries=$((tries + 1)); [[ $tries -ge 30 ]] && return 1; sleep 0.5
  done
  tmux pipe-pane -t "$SERVER_PANE" 2>/dev/null || true
  tmux pipe-pane -t "$SERVER_PANE" "cat >> '${logfile}'"
}

# 用 start.sh 起整栈；据实导出 start.sh 支持的 env（不臆造）。
launch_stack() {
  local logfile="$1" mock_flag=()
  [[ "$MOCK_AGENT" == "1" ]] && mock_flag=(--mock)
  # 关键：BONG_NPC_NO_DORMANT 必须为 0，否则 dormant 不 seed（start.sh:66 默认 0）。
  BONG_DORMANT_ROGUE_SEED_COUNT="$DORMANT_COUNT" \
  BONG_ROGUE_SEED_COUNT="$ROGUE_COUNT" \
  BONG_NPC_NO_DORMANT=0 \
    bash "$START_SH" "${mock_flag[@]}" >/dev/null
  attach_server_pipe "$logfile" || fatal "无法 attach server pane ($SERVER_PANE) 的 pipe-pane"
  if [[ "$KILL_AGENT_PANE" == "1" ]]; then
    tmux kill-pane -t "$AGENT_PANE" 2>/dev/null || true
  fi
}

stop_stack() {
  tmux pipe-pane -t "$SERVER_PANE" 2>/dev/null || true
  tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
  # 等 25565 释放，避免重启撞 address-in-use
  local waited=0
  while port_in_use; do
    sleep 1; waited=$((waited + 1)); [[ $waited -ge 20 ]] && break
  done
}

# 轮询直到 hlen == 目标，或超时。期间周期打印进度（含构建等待）。
wait_for_hlen() {
  local target="$1" timeout="$2" waited=0 cur
  while :; do
    cur="$(hlen_dormant)"; [[ -z "$cur" ]] && cur=0
    [[ "$cur" == "$target" ]] && return 0
    [[ $waited -ge $timeout ]] && return 1
    if (( waited % 30 == 0 )); then
      info "$(c_dim "等待中… 已 ${waited}s / ${timeout}s，当前 hlen=${cur}（目标 ${target}；冷启动含 release 构建）")"
    fi
    sleep 5; waited=$((waited + 5))
  done
}

# 轮询日志直到出现某串，或超时。
wait_for_log() {
  local logfile="$1" pat="$2" timeout="$3" waited=0
  while :; do
    grep -qF "$pat" "$logfile" 2>/dev/null && return 0
    [[ $waited -ge $timeout ]] && return 1
    if (( waited % 30 == 0 )); then
      info "$(c_dim "等待日志 '${pat}'… 已 ${waited}s / ${timeout}s")"
    fi
    sleep 5; waited=$((waited + 5))
  done
}

count_in_log() { grep -cF "$2" "$1" 2>/dev/null || true; }

# ----------------------------------------------------------------------------
# 4. 预检
# ----------------------------------------------------------------------------
section "预检"
for tool in tmux cargo redis-cli redis-server awk; do
  command -v "$tool" >/dev/null 2>&1 || fatal "缺少依赖: ${tool}"
done
info "依赖齐全: tmux / cargo / redis-cli / redis-server / awk"
[[ -f "$START_SH" ]] || fatal "找不到 start.sh: ${START_SH}"
info "start.sh: ${START_SH}"
info "日志目录: ${LOG_DIR}"

# Redis 必须先在线（要在 start.sh 之前 flushall 并自管控）。不在线则尝试拉起。
if ! redis_alive; then
  info "Redis 未在线，尝试 redis-server --daemonize yes …"
  redis-server --daemonize yes --save '' --appendonly no >/dev/null 2>&1 || true
  waited=0
  until redis_alive; do
    sleep 1; waited=$((waited + 1)); [[ $waited -ge 15 ]] && fatal "Redis 仍无法连接"
  done
fi
info "Redis 在线: $(rc ping 2>/dev/null)"

# 收掉可能残留的旧 bong 会话
stop_stack

# ----------------------------------------------------------------------------
# 5. flushall 确认（破坏性：清空整个 Redis，正是「空 Redis 冷启动」前置）
# ----------------------------------------------------------------------------
section "冷启动前置：flushall"
printf '  %s 本步将执行 redis-cli FLUSHALL，清空全部 Redis 数据（含 agent WorldModel）。\n' "$(c_red '[WARN]')"
printf '         这是 plan §一 step 1「空 Redis 强制全量冷 seed」的必要前置。\n'
if [[ "$ASSUME_YES" != "1" ]]; then
  if [[ -t 0 ]]; then
    read -r -p "         确认清空 Redis 并继续？[y/N] " ans
    [[ "$ans" =~ ^[Yy]$ ]] || fatal "用户取消（未确认 flushall）"
  else
    fatal "非交互环境且未带 --yes / VERIFY_ASSUME_YES=1，拒绝自动 flushall"
  fi
fi
rc flushall >/dev/null
[[ "$(rc dbsize 2>/dev/null | tr -d '[:space:]')" == "0" ]] || fatal "flushall 后 dbsize 非 0"
info "Redis 已清空 (dbsize=0)"

# ----------------------------------------------------------------------------
# 6. Phase 1 — 冷启动 + 断言 [1][2]
# ----------------------------------------------------------------------------
section "Phase 1 · 冷启动 (dormant=${DORMANT_COUNT}, rogue=${ROGUE_COUNT})"
info "启动: BONG_DORMANT_ROGUE_SEED_COUNT=${DORMANT_COUNT} BONG_ROGUE_SEED_COUNT=${ROGUE_COUNT} bash start.sh $([[ "$MOCK_AGENT" == 1 ]] && echo --mock)"
launch_stack "$LOG_COLD"
info "已起栈，server 日志 → ${LOG_COLD}（冷启动首跑需 release 构建，请耐心）"

if wait_for_hlen "$DORMANT_COUNT" "$BUILD_TIMEOUT"; then
  cold_hlen="$(hlen_dormant)"
  record PASS "hlen ${DORMANT_KEY} == ${DORMANT_COUNT}" \
    "实测 hlen=${cold_hlen}；说明 1000 条 dormant 快照真的写进了 Redis HASH（P2 写慢已根治）"
else
  cold_hlen="$(hlen_dormant)"
  record FAIL "hlen ${DORMANT_KEY} == ${DORMANT_COUNT}" \
    "${BUILD_TIMEOUT}s 内只到 hlen=${cold_hlen}（期望 ${DORMANT_COUNT}）；写未成功——查 ${LOG_COLD}（构建失败？dormant 写仍超时？）"
fi

# [2a] 无泄漏 temp key
tmp_now="$(scan_count "$TMP_GLOB")"
tmp_now_literal="$(scan_count "$TMP_GLOB_LITERAL")"
if [[ "$tmp_now" == "0" ]]; then
  record PASS "无泄漏 temp key (${TMP_GLOB})" \
    "scan '${TMP_GLOB}'=0（字面 '${TMP_GLOB_LITERAL}'=${tmp_now_literal}）；bug ② 已止，写完即 RENAME 不留残骸"
else
  record FAIL "无泄漏 temp key (${TMP_GLOB})" \
    "scan '${TMP_GLOB}'=${tmp_now}（期望 0）；temp key 仍在泄漏——超时分支未清理 / 非确定性 key 回归"
fi

# [2b] 日志无 reconnect / hash timeout 刷屏
rc_reconnect="$(count_in_log "$LOG_COLD" "$PAT_RECONNECT")"
rc_timeout="$(count_in_log "$LOG_COLD" "$PAT_HASH_TIMEOUT")"
if [[ "$rc_reconnect" == "0" && "$rc_timeout" == "0" ]]; then
  record PASS "出站桥无故障刷屏" \
    "'reconnect attempt='=0 且 'timed out replacing hash'=0；bug ① 已止，dormant 写不再饿死/重连风暴"
else
  record FAIL "出站桥无故障刷屏" \
    "'reconnect attempt='=${rc_reconnect}，'timed out replacing hash'=${rc_timeout}（均期望 0）；出站桥被 dormant 写拖垮——查 ${LOG_COLD}"
fi

# ----------------------------------------------------------------------------
# 7. Phase 2 — 保留 Redis 重启 + 断言 [3]
# ----------------------------------------------------------------------------
section "Phase 2 · 保留 Redis 重启（验证恢复，不 re-seed）"
pre_restart_hlen="$(hlen_dormant)"
info "重启前 hlen=${pre_restart_hlen}（Redis 数据保留，不 flushall）"
stop_stack
launch_stack "$LOG_RESTART"
info "已重启，server 日志 → ${LOG_RESTART}"

if wait_for_log "$LOG_RESTART" "$PAT_RESTORE" "$RESTART_TIMEOUT"; then
  restore_line="$(grep -F "$PAT_RESTORE" "$LOG_RESTART" | head -1 | sed 's/^[[:space:]]*//')"
  record PASS "重启从 Redis HASH 恢复 dormant" \
    "日志命中: ${restore_line}"
  # 恢复成功就不该再全量 seed
  seed_after="$(count_in_log "$LOG_RESTART" "$PAT_SEED")"
  if [[ "$seed_after" == "0" ]]; then
    record PASS "重启未 re-seed dormant" \
      "重启日志中 'dormant rogue NPC snapshots'=0；走的是恢复分支（is_empty()==false），符合预期"
  else
    record FAIL "重启未 re-seed dormant" \
      "重启日志中 seed 串出现 ${seed_after} 次（期望 0）；恢复后又全量 seed——is_empty 守卫失效，会重复散布"
  fi
else
  record FAIL "重启从 Redis HASH 恢复 dormant" \
    "${RESTART_TIMEOUT}s 内未见 '${PAT_RESTORE}'；恢复路径未触发——查 ${LOG_RESTART}"
fi

# 重启后数据应仍完好
post_hlen="$(hlen_dormant)"
post_tmp="$(scan_count "$TMP_GLOB")"
if [[ "$post_hlen" == "$DORMANT_COUNT" && "$post_tmp" == "0" ]]; then
  record PASS "重启后数据完好" \
    "hlen=${post_hlen}==${DORMANT_COUNT} 且 temp key=0；恢复未损坏/未膨胀数据"
else
  record FAIL "重启后数据完好" \
    "hlen=${post_hlen}（期望 ${DORMANT_COUNT}），temp key=${post_tmp}（期望 0）；恢复破坏了数据集"
fi

# ----------------------------------------------------------------------------
# 8. Phase 3 — 长跑内存平稳 + 断言 [4]
# ----------------------------------------------------------------------------
if [[ "${MEM_WINDOW}" -gt 0 ]]; then
  section "Phase 3 · 长跑内存平稳 (窗口 ${MEM_WINDOW}s, 采样间隔 ${MEM_INTERVAL}s)"
  mem_base="$(used_memory_bytes)"; [[ -z "$mem_base" ]] && mem_base=0
  mem_max="$mem_base"
  tmp_spike=0
  elapsed=0
  info "基线 used_memory=${mem_base} bytes"
  while [[ $elapsed -lt $MEM_WINDOW ]]; do
    sleep "$MEM_INTERVAL"; elapsed=$((elapsed + MEM_INTERVAL))
    m="$(used_memory_bytes)"; [[ -z "$m" ]] && m=0
    t="$(scan_count "$TMP_GLOB")"
    [[ "$m" -gt "$mem_max" ]] && mem_max="$m"
    [[ "$t" != "0" ]] && tmp_spike=$((tmp_spike + 1))
    info "$(c_dim "t=${elapsed}s used_memory=${m} bytes, temp_key=${t}")"
  done
  # 死亡螺旋特征：内存单调暴涨（每次失败重试漏 ~5MB）+ temp key 持续累积。
  # 容差：峰值 ≤ 基线 + max(基线*50%, 32MB)，足以放过正常波动、绝对挡住 467MB 级膨胀。
  slack=$(( mem_base / 2 ))
  [[ $slack -lt 33554432 ]] && slack=33554432   # 至少 32MiB
  threshold=$(( mem_base + slack ))
  if [[ "$mem_max" -le "$threshold" && "$tmp_spike" == "0" ]]; then
    record PASS "长跑内存平稳（无死亡螺旋）" \
      "峰值 used_memory=${mem_max} ≤ 阈值 ${threshold} bytes，且 temp key 全程为 0"
  else
    record FAIL "长跑内存平稳（无死亡螺旋）" \
      "峰值 used_memory=${mem_max}（阈值 ${threshold}），temp key 越界采样次数=${tmp_spike}；疑似 temp key 泄漏导致内存膨胀"
  fi
else
  section "Phase 3 · 长跑内存平稳 [SKIPPED]"
  info "MEM_WINDOW=0，跳过内存窗口"
fi

# ----------------------------------------------------------------------------
# 9. 汇总
# ----------------------------------------------------------------------------
section "汇总"
for line in "${RESULT_LINES[@]}"; do
  v="${line%%|*}"; t="${line#*|}"
  if [[ "$v" == "PASS" ]]; then printf '  %s  %s\n' "$(c_green '[PASS]')" "$t"; else printf '  %s  %s\n' "$(c_red '[FAIL]')" "$t"; fi
done
echo
if [[ "$FAILURES" -eq 0 ]]; then
  printf '%s 全部通过 —— 1000 dormant 空 Redis 冷启动 + 重启恢复 + 内存平稳，端到端绿。\n' "$(c_green '[RESULT] PASS')"
  exit 0
else
  printf '%s %d 项失败 —— 详见上方各项细节与日志 (%s)。\n' "$(c_red '[RESULT] FAIL')" "$FAILURES" "$LOG_DIR"
  exit 1
fi
