#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JAVA17_HOME=""
if [[ -d "$HOME/.sdkman/candidates/java/17.0.18-amzn" ]]; then
  JAVA17_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn"
else
  JAVA17_HOME="${JAVA_HOME:-}"
fi
if [[ -n "$JAVA17_HOME" ]]; then
  export JAVA_HOME="$JAVA17_HOME"
  export PATH="$JAVA_HOME/bin:$PATH"
fi

cd "$ROOT_DIR/client"
./gradlew --no-daemon test --tests "com.bong.client.hud.HudImmersionMatrixTest"
