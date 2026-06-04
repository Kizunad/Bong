#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "[coffin-e2e] server coffin registry and lifecycle tests"
(
  cd "$ROOT/server"
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test coffin::tests -- --test-threads=1
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test player_lifespan_load_applies_coffin_offline_multiplier -- --test-threads=1
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test v23_migration_adds_in_coffin_to_legacy_player_lifespan_table -- --test-threads=1
)

echo "[coffin-e2e] P4 四档倍率 / 配方 / 材料 / 平衡梯度"
(
  cd "$ROOT/server"
  # P4: 3 档新配方注册 + 材料 + 守恒 + set_grade + 梯度
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test coffin::tests::p4_ -- --test-threads=1
  # P4: /coffin grade dev 命令解析 + 档级直写
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test cmd::dev::coffin::tests -- --test-threads=1
  # P4: supply_coffin loot 表条数（含新增卷轴）
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test supply_coffin::tests::loot_tables_have_expected_entry_counts -- --test-threads=1
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test supply_coffin::tests::loot_tables_have_no_duplicate_template_ids_within_grade -- --test-threads=1
  # P4: workbench 守恒 pin（含 coffin 配方）
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo test coffin::tests::p4_spirit_quality_conservation_trivial -- --test-threads=1
)

echo "[coffin-e2e] schema coffin wire contract"
(
  cd "$ROOT/agent"
  npm test -w @bong/schema -- coffin
)

echo "[coffin-e2e] client coffin protocol, state, and HUD"
(
  cd "$ROOT/client"
  JAVA17_HOME="${BONG_JAVA17_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
  export JAVA_HOME="$JAVA17_HOME"
  export PATH="$JAVA17_HOME/bin:$PATH"
  ./gradlew test \
    --tests "com.bong.client.network.ClientRequestProtocolTest.encodesCoffinLifecycleRequests" \
    --tests "com.bong.client.network.ClientRequestSenderTest.sendCoffinLifecycleUsesCorrectChannelAndJson" \
    --tests "com.bong.client.network.CoffinStateHandlerTest" \
    --tests "com.bong.client.hud.CoffinHudPlannerTest" \
    --tests "com.bong.client.hud.HudLayoutPresetTest.coffinHudUsesBarsWidget"
)

echo "[coffin-e2e] ok"
