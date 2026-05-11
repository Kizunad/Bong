#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

is_java17() {
  [[ -n "${1:-}" && -x "$1/bin/java" ]] && "$1/bin/java" -version 2>&1 | grep -q 'version "17\.'
}

JAVA17_HOME=""
for candidate in "${JAVA_HOME:-}" "$HOME/.sdkman/candidates/java/17.0.18-amzn" "/usr/lib/jvm/java-17-openjdk-amd64"; do
  if is_java17 "$candidate"; then
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

(
  cd "$ROOT_DIR/client"
  ./gradlew --no-daemon test --tests "com.bong.client.season.SeasonFullExperienceTest"
)

(
  cd "$ROOT_DIR/server"
  cargo test fauna::migration
  cargo test npc::seasonal_behavior
)

(
  cd "$ROOT_DIR/agent"
  npm run build -w @bong/schema
  npm test -w @bong/tiandao -- seasonal-narration.test.ts
)
