#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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
  echo "scripts/breakthrough_cinematic_test.sh requires Java 17, got:" >&2
  echo "$JAVA_VERSION_OUTPUT" >&2
  exit 1
fi

cd "$ROOT_DIR/server"
cargo test breakthrough_cinematic::tests::

cd "$ROOT_DIR/client"
./gradlew --no-daemon test \
  --tests "com.bong.client.cultivation.BreakthroughSpectacleRendererTest" \
  --tests "com.bong.client.network.BreakthroughCinematicHandlerTest"

cd "$ROOT_DIR/agent"
npm test -w @bong/schema
npm test -w @bong/tiandao -- breakthrough-cinematic-narration
