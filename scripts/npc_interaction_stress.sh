#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_server_test() {
  local filter="$1"
  echo "=== server: ${filter} ==="
  (
    cd "$ROOT_DIR/server"
    CARGO_PROFILE_TEST_DEBUG="${CARGO_PROFILE_TEST_DEBUG:-0}" \
      cargo test -j1 "$filter" -- --test-threads=1
  )
}

JAVA17_HOME=""
for candidate in "$HOME/.sdkman/candidates/java/17.0.18-amzn" "/usr/lib/jvm/java-17-openjdk-amd64" "${JAVA_HOME:-}"; do
  if [[ -n "$candidate" && -x "$candidate/bin/java" ]]; then
    JAVA17_HOME="$candidate"
    break
  fi
done

if [[ -z "$JAVA17_HOME" ]]; then
  echo "JDK 17 not found; set JAVA_HOME to a Java 17 installation." >&2
  exit 1
fi

export JAVA_HOME="$JAVA17_HOME"
export PATH="$JAVA_HOME/bin:$PATH"

JAVA_VERSION_OUTPUT="$("$JAVA_HOME/bin/java" -version 2>&1)"
if ! grep -q 'version "17\.' <<< "$JAVA_VERSION_OUTPUT"; then
  echo "scripts/npc_interaction_stress.sh requires Java 17, got:" >&2
  echo "$JAVA_VERSION_OUTPUT" >&2
  exit 1
fi

run_server_test "network::npc_bubble"
run_server_test "network::npc_mood"
run_server_test "network::tsy_polish"
run_server_test "npc::interaction_memory"
run_server_test "npc::tsy_hostile::tests::dao_chang_lure_flip_timing"
run_server_test "npc::tsy_hostile::tests::obsession_high_value_lure_opens_short_release_window"

echo "=== client: NPC/TSY interaction HUD matrix ==="
(
  cd "$ROOT_DIR/client"
  ./gradlew --no-daemon test \
    --tests "com.bong.client.npc.*" \
    --tests "com.bong.client.tsy.*" \
    --tests "com.bong.client.hud.TargetInfoHudPlannerTest" \
    --tests "com.bong.client.hud.BongHudOrchestratorTest"
)
