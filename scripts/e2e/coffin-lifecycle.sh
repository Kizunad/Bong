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

echo "[coffin-e2e] schema coffin wire contract"
(
  cd "$ROOT/agent"
  npm test -w @bong/schema -- coffin
)

echo "[coffin-e2e] client coffin protocol, state, and HUD"
(
  cd "$ROOT/client"
  JAVA17_HOME="${BONG_JAVA17_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
  JAVA_HOME="$JAVA17_HOME"
  PATH="$JAVA17_HOME/bin:$PATH"
  ./gradlew test \
    --tests "com.bong.client.network.ClientRequestProtocolTest.encodesCoffinLifecycleRequests" \
    --tests "com.bong.client.network.ClientRequestSenderTest.sendCoffinLifecycleUsesCorrectChannelAndJson" \
    --tests "com.bong.client.network.CoffinStateHandlerTest" \
    --tests "com.bong.client.hud.CoffinHudPlannerTest" \
    --tests "com.bong.client.hud.HudLayoutPresetTest.coffinHudUsesBarsWidget"
)

echo "[coffin-e2e] ok"
