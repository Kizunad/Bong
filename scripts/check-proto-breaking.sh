#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE_REF="${BASE_REF:?BASE_REF is required}"
ALLOWLIST="$ROOT/proto/buf-breaking-approvals.tsv"

git -C "$ROOT" fetch --no-tags --depth=1 origin \
  "+$BASE_REF:refs/remotes/origin/$BASE_REF"
base_commit="$(git -C "$ROOT" rev-parse --verify "refs/remotes/origin/$BASE_REF^{commit}")"
proto_type="$(git -C "$ROOT" cat-file -t "$base_commit:proto" 2>/dev/null || true)"

if [ "$proto_type" = "tree" ]; then
  report="$(mktemp)"
  buf_stderr="$(mktemp)"
  trap 'rm -f "$report" "$buf_stderr"' EXIT

  # 先完整执行 buf；只有明确登记的 finding 才允许继续。
  set +e
  (
    cd "$ROOT/proto"
    buf breaking --error-format=json --against "../.git#ref=$base_commit,subdir=proto" >"$report" 2>"$buf_stderr"
  )
  buf_status=$?
  set -e

  if [ "$buf_status" -eq 0 ]; then
    cat "$buf_stderr" >&2
    exit 0
  fi

  # JSON 逐行解析，避免用文本 grep 把未批准的破坏性变更吞掉。
  if python3 - "$report" "$ALLOWLIST" <<'PY'
import json
import hashlib
import pathlib
import re
import sys

report_path = pathlib.Path(sys.argv[1])
allowlist_path = pathlib.Path(sys.argv[2])

approved = set()
if allowlist_path.is_file():
    for line_number, raw_line in enumerate(allowlist_path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        columns = raw_line.split("\t", 3)
        if len(columns) != 4 or not all(column.strip() for column in columns):
            print(f"协议 breaking 审批清单第 {line_number} 行格式无效：需要 type/path/fingerprint/reason 四列", file=sys.stderr)
            sys.exit(1)
        finding_type, path, fingerprint, _reason = columns
        if not re.fullmatch(r"[0-9a-f]{64}", fingerprint):
            print(f"协议 breaking 审批清单第 {line_number} 行指纹不是 64 位小写 SHA-256", file=sys.stderr)
            sys.exit(1)
        key = (finding_type, path, fingerprint)
        if key in approved:
            print(f"协议 breaking 审批清单第 {line_number} 行重复：{finding_type} {path}", file=sys.stderr)
            sys.exit(1)
        approved.add(key)

raw_findings = [line for line in report_path.read_text(encoding="utf-8").splitlines() if line.strip()]
if not raw_findings:
    print("buf breaking 失败但没有结构化 finding，拒绝把工具错误当成通过", file=sys.stderr)
    sys.exit(1)

findings = []
for line_number, raw_line in enumerate(raw_findings, 1):
    try:
        finding = json.loads(raw_line)
    except json.JSONDecodeError as error:
        print(f"buf breaking 输出第 {line_number} 行不是合法 JSON：{error}", file=sys.stderr)
        sys.exit(1)
    if not isinstance(finding, dict) or not all(key in finding for key in ("type", "path", "message")):
        print(f"buf breaking 输出第 {line_number} 行缺少 type/path/message", file=sys.stderr)
        sys.exit(1)
    findings.append(finding)

unapproved = []
for finding in findings:
    message = finding["message"]
    fingerprint = hashlib.sha256(
        "\0".join((finding["type"], finding["path"], message)).encode("utf-8")
    ).hexdigest()
    key = (finding["type"], finding["path"], fingerprint)
    if key in approved:
        print(f"已批准协议删除：{finding['type']} {finding['path']} — {finding['message']}")
    else:
        unapproved.append(finding)

if unapproved:
    for finding in unapproved:
        print(f"未批准的 proto breaking：{finding['type']} {finding['path']} — {finding['message']}", file=sys.stderr)
    sys.exit(1)
PY
  then
    cat "$buf_stderr" >&2
    exit 0
  fi

  cat "$buf_stderr" >&2
  exit 1
elif [ -z "$proto_type" ]; then
  echo "proto/ not found on verified base commit $base_commit — skipping breaking check (first PR)"
else
  echo "verified base path proto has unexpected git object type: $proto_type" >&2
  exit 1
fi
