#!/usr/bin/env bash
# Retry a gradle invocation ONLY on transient network / rate-limit failures
# (Maven Central / plugins.gradle.org / services.gradle.org returning HTTP 429,
# connection resets, read timeouts). A genuine build failure — test assertion,
# missing dependency (404), proto contract break — must fail immediately.
#
# Evidence (2026-08-08, run 31248889752): gradle 429'd during fabric-loom
# transitive resolution on BOTH repo.maven.apache.org and plugins.gradle.org,
# dying pre-suite in ~50s ("Received status code 429 from server: Too Many
# Requests"). A blanket job-level retry would re-run every stage; this retries
# only the gradle step, and only when the failure log carries a rate-limit /
# transient-network signature.
set -u

MAX_ATTEMPTS="${GRADLE_RETRY_MAX_ATTEMPTS:-3}"
BASE_SLEEP="${GRADLE_RETRY_BASE_SLEEP:-30}"

# Deliberately narrow: no bare "Could not resolve", no "Could not find", no 404 —
# those are genuine resolution errors and must fail the build. Only HTTP 429 /
# rate-limit / transport-level failures are retried.
RETRY_RE='(Too many requests|status code 429|HTTP/1\.1 429|429 Too Many|Connection reset|SERVER ERROR|Read timed out|SocketTimeoutException|handshake timed out|peer not authenticated)'

log="$(mktemp /tmp/gradle-retry.XXXXXX.log)"
trap 'rm -f "$log"' EXIT

last_code=1
for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
  echo "gradle attempt $attempt/$MAX_ATTEMPTS: $*"
  "$@" >"$log" 2>&1
  last_code=$?
  if [ "$last_code" -eq 0 ]; then
    cat "$log"
    exit 0
  fi
  cat "$log" >&2
  if ! grep -aqE "$RETRY_RE" "$log"; then
    echo "gradle failed (exit $last_code) with no transient-network signature — failing immediately" >&2
    exit "$last_code"
  fi
  if [ "$attempt" -lt "$MAX_ATTEMPTS" ]; then
    sleep_for=$((BASE_SLEEP * attempt))
    echo "transient network/rate-limit detected — retrying in ${sleep_for}s (attempt $((attempt+1))/$MAX_ATTEMPTS)" >&2
    sleep "$sleep_for"
  fi
done
echo "gradle still failing after $MAX_ATTEMPTS attempts — last exit $last_code" >&2
exit "$last_code"
