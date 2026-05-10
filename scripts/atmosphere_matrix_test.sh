#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/client"

is_java17() {
  [ -x "$1/bin/java" ] && "$1/bin/java" -version 2>&1 | grep -q 'version "17\.'
}

if [ -n "${JAVA_HOME:-}" ] && is_java17 "$JAVA_HOME"; then
  export PATH="$JAVA_HOME/bin:$PATH"
elif is_java17 "$HOME/.sdkman/candidates/java/current"; then
  export JAVA_HOME="$HOME/.sdkman/candidates/java/current"
  export PATH="$JAVA_HOME/bin:$PATH"
else
  JAVA17_HOME=""
  for candidate in "$HOME"/.sdkman/candidates/java/17.*; do
    if is_java17 "$candidate"; then
      JAVA17_HOME="$candidate"
      break
    fi
  done
  if [ -n "$JAVA17_HOME" ]; then
    export JAVA_HOME="$JAVA17_HOME"
    export PATH="$JAVA_HOME/bin:$PATH"
  elif command -v java >/dev/null 2>&1 && java -version 2>&1 | grep -q 'version "17\.'; then
    unset JAVA_HOME
  else
    echo "Java 17 is required for Fabric client tests; set JAVA_HOME to a JDK 17 install." >&2
    exit 1
  fi
fi

./gradlew test --tests com.bong.client.atmosphere.ZoneAtmosphereTest
