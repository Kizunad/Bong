#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/client"

./gradlew test \
  --tests '*ScreenTransitionTest' \
  --tests '*LoadingOverlayTest' \
  --tests '*ConnectionStatusIndicatorTest' \
  --tests '*UiTransitionPerformanceTest'
