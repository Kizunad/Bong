#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/client"

if [[ -z "${JAVA_HOME:-}" ]]; then
  echo "ERROR: JAVA_HOME is not set. Use JDK 17 for Fabric client validation." >&2
  exit 1
fi

if [[ ! -x "$JAVA_HOME/bin/java" ]] || ! "$JAVA_HOME/bin/java" -version 2>&1 | grep -q 'version "17\.'; then
  echo "ERROR: JAVA_HOME must point to JDK 17: $JAVA_HOME" >&2
  exit 1
fi

./gradlew test \
  --tests '*ScreenTransitionTest' \
  --tests '*LoadingOverlayTest' \
  --tests '*ConnectionStatusIndicatorTest' \
  --tests '*UiTransitionPerformanceTest'
