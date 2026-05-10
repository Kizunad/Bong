#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/client"

if [ -d "$HOME/.sdkman/candidates/java/17.0.18-amzn" ]; then
  export JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn"
  export PATH="$JAVA_HOME/bin:$PATH"
else
  echo "Java 17 not found at $HOME/.sdkman/candidates/java/17.0.18-amzn; refusing to run Fabric tests with system Java." >&2
  exit 1
fi

./gradlew test --tests com.bong.client.atmosphere.ZoneAtmosphereTest
