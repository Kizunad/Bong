#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/client"

if [ -d "$HOME/.sdkman/candidates/java/17.0.18-amzn" ]; then
  export JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn"
  export PATH="$JAVA_HOME/bin:$PATH"
fi

./gradlew test --tests com.bong.client.atmosphere.ZoneAtmosphereTest
