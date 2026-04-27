#!/usr/bin/env bash
# plan-tsy-lifecycle-v1 §8 — TSY 生命周期自动化 smoke 脚本
#
# 跑：server cargo test (含 lifecycle 单测 + integration test) + schema vitest +
# grep 校验 lifecycle 模块的关键 system 都注册到 world::register。
#
# 与 smoke-tsy-loot.sh 同样不跑 cargo clippy / fmt-check —— main 上 dead-code
# warning 已知红，由后续 plan 单独清理；本脚本只校验"lifecycle 路径全绿"。
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "[smoke-tsy-lifecycle] running cargo fmt --check (server)..."
(cd server && cargo fmt --check)

echo "[smoke-tsy-lifecycle] running cargo test (server, full suite ~1300+ tests)..."
(cd server && cargo test --bin bong-server)

echo "[smoke-tsy-lifecycle] running schema vitest + check..."
(cd agent/packages/schema && npm test && npm run check)

echo "[smoke-tsy-lifecycle] verifying lifecycle systems are registered..."
register_block=$(awk '/pub fn register\(app: &mut App\)/,/^}/' server/src/world/mod.rs)
for needle in 'tsy_lifecycle::register'; do
  if ! grep -q "$needle" <<<"$register_block"; then
    echo "❌ world::register 缺 $needle"
    exit 1
  fi
done

echo "[smoke-tsy-lifecycle] verifying NpcArchetype::Daoxiang variant + loot table..."
if ! grep -q 'NpcArchetype::Daoxiang' server/src/npc/loot.rs; then
  echo "❌ loot table 漏 NpcArchetype::Daoxiang"
  exit 1
fi
if ! grep -q 'Daoxiang,' server/src/npc/lifecycle.rs; then
  echo "❌ NpcArchetype enum 漏 Daoxiang variant"
  exit 1
fi

echo "[smoke-tsy-lifecycle] verifying schema bridge has 4 new lifecycle events..."
for needle in TsyZoneActivatedV1 TsyCollapseStartedV1 TsyCollapseCompletedV1 DaoxiangSpawnedV1; do
  if ! grep -q "$needle" agent/packages/schema/src/schema-registry.ts; then
    echo "❌ schema-registry 缺 $needle"
    exit 1
  fi
done

echo "[smoke-tsy-lifecycle] all green ✅"
