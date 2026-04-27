#!/usr/bin/env bash
# plan-tsy-zone-v1 §6 — automated smoke script
#
# 跑一遍 server 全测 + schema 全测，确认 TSY zone 基础设施保持绿色。
# 不跑 cargo clippy / fmt-check：这两项目前在 main 上即处于已知红状态
# （npc/territory.rs 等 41 个不相关 unused-id 警告），由后续 plan 单独清理。
# 本脚本只校验"TSY 路径所有测试都通过 + 生成 artifact 仍 fresh"。
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "[smoke-tsy-zone] running cargo test (server)..."
(cd server && cargo test --bin bong-server)

echo "[smoke-tsy-zone] running schema vitest + check..."
(cd agent/packages/schema && npm test && npm run check)

echo "[smoke-tsy-zone] all green ✅"
