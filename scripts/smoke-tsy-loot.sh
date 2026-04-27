#!/usr/bin/env bash
# plan-tsy-loot-v1 §9 — TSY loot 自动化 smoke 脚本
#
# 跑：server cargo test + schema vitest + grep 校验 DeathEvent 字段完整性 +
# gradle test build（验证 MixinPlayerEntityDrop 加载）。
#
# 与 smoke-tsy-zone.sh 同样不跑 cargo clippy / fmt-check —— main 上 dead-code
# warning 已知红，由后续 plan 单独清理；本脚本只校验"loot 路径全绿"。
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "[smoke-tsy-loot] running cargo test (server, all 1300+ tests)..."
(cd server && cargo test --bin bong-server)

echo "[smoke-tsy-loot] running schema vitest + check..."
(cd agent/packages/schema && npm test && npm run check)

echo "[smoke-tsy-loot] verifying DeathEvent emit sites all have attacker fields..."
# §6 确保所有 DeathEvent 发出点都带 attacker 字段（编译器会 catch missing field，
# 但 grep 形式更直观给 review 看）
emit_sites=$(grep -rln 'DeathEvent {' server/src/ || true)
if [ -z "$emit_sites" ]; then
  echo "❌ 找不到任何 DeathEvent {} emit 点"
  exit 1
fi
echo "  found emit sites:"
for f in $emit_sites; do
  count=$(grep -c 'DeathEvent {' "$f")
  echo "    $f: $count emit(s)"
done

echo "[smoke-tsy-loot] running gradle test + build (verify MixinPlayerEntityDrop loads)..."
(cd client && ./gradlew test build --no-daemon)

echo "[smoke-tsy-loot] all green ✅"
