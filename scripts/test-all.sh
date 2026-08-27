#!/usr/bin/env bash
# 三栈 native test 编排入口；只调度既有命令，不接管测试语义。
set -uo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
OWNERS_FILE="$SCRIPT_DIR/test-all-owners.tsv"
CALLER_PWD="$(pwd -P)"

PROFILE=unit
REPORT_ARG=
CONTINUE=0
LIST=0
HELP=0
REQUESTED=()
DEFAULTS=
SELECTED=()

declare -a OWNER_ORDER=(server client schema tiandao scripts)
declare -A OWNER_ROLE REVIEWER_PATH EVIDENCE_PATH

usage() {
    cat <<'EOF'
用法：scripts/test-all.sh [选项]
  --profile unit|contract|full|e2e|preview
  --suite server|client|schema|tiandao|scripts （可重复）
  --report-dir DIR       指定本次 run-private 报告目录
  --continue              前一 suite 失败后继续，最终仍返回非零
  --list                  只打印并校验 owners 映射，不执行测试
  --help

默认 profile 为 unit，按串行 DAG 执行 schema → server → client → tiandao。
unit 只含 server/client/schema/tiandao；scripts 属于 contract。
unit/full 不启动 Redis、真实 LLM、BongWorldGen 或 raster 生成。
preview 必须由调用方提供 BONG_TERRAIN_RASTER_DIR 或 BONG_TERRAIN_RASTER_PATH，
以及 BONG_CLIENT_PREVIEW_DIR。
EOF
}

usage_error() {
    printf 'test-all: %s\n' "$1" >&2
    usage >&2
    exit 2
}

while (($# > 0)); do
    case "$1" in
        --profile)
            (($# > 1)) || usage_error "--profile 缺少值"
            PROFILE="$2"
            shift 2
            ;;
        --suite)
            (($# > 1)) || usage_error "--suite 缺少值"
            REQUESTED+=("$2")
            shift 2
            ;;
        --report-dir)
            (($# > 1)) || usage_error "--report-dir 缺少值"
            REPORT_ARG="$2"
            shift 2
            ;;
        --continue) CONTINUE=1; shift ;;
        --list) LIST=1; shift ;;
        --help|-h) HELP=1; shift ;;
        *) usage_error "未知参数：$1" ;;
    esac
done

((HELP == 1)) && { usage; exit 0; }
case "$PROFILE" in
    unit|contract|full|e2e|preview) ;;
    *) usage_error "未知 profile：$PROFILE" ;;
esac

validate_repo_path() {
    local path="$1"
    local clean="${path%/}"
    [[ -n "$clean" && "$clean" != /* ]] || return 1
    [[ "$clean" != *$'\n'* && "$clean" != *$'\r'* ]] || return 1
    case "/$clean/" in
        */../*|*/./*|*//* ) return 1 ;;
    esac
    [[ -e "$ROOT/$clean" ]]
}

load_owners() {
    local line="" line_no=0 rows=0
    local suite="" role="" reviewer="" evidence="" extra=""
    local header=$'suite\towner_role\treviewer_path\tevidence'
    [[ -r "$OWNERS_FILE" ]] || {
        printf 'test-all: owners 映射不可读：%s\n' "$OWNERS_FILE" >&2
        return 1
    }
    while IFS= read -r line || [[ -n "$line" ]]; do
        line_no=$((line_no + 1))
        if ((line_no == 1)); then
            [[ "$line" == "$header" ]] || {
                printf 'test-all: owners header 不匹配：%s\n' "$line" >&2
                return 1
            }
            continue
        fi
        [[ -n "$line" ]] || {
            printf 'test-all: owners 第 %d 行为空\n' "$line_no" >&2
            return 1
        }
        suite=; role=; reviewer=; evidence=; extra=
        IFS=$'\t' read -r suite role reviewer evidence extra <<< "$line"
        [[ -n "$suite" && -n "$role" && -n "$reviewer" && -n "$evidence" && -z "$extra" ]] || {
            printf 'test-all: owners 第 %d 行必须恰好四列\n' "$line_no" >&2
            return 1
        }
        case "$suite" in
            server|client|schema|tiandao|scripts) ;;
            *) printf 'test-all: 未知 owner suite：%s\n' "$suite" >&2; return 1 ;;
        esac
        [[ -v "OWNER_ROLE[$suite]" ]] && {
            printf 'test-all: owner suite 重复：%s\n' "$suite" >&2
            return 1
        }
        validate_repo_path "$reviewer" || {
            printf 'test-all: reviewer_path 不存在或越界：%s\n' "$reviewer" >&2
            return 1
        }
        validate_repo_path "$evidence" || {
            printf 'test-all: evidence path 不存在或越界：%s\n' "$evidence" >&2
            return 1
        }
        OWNER_ROLE["$suite"]="$role"
        REVIEWER_PATH["$suite"]="$reviewer"
        EVIDENCE_PATH["$suite"]="$evidence"
        rows=$((rows + 1))
    done < "$OWNERS_FILE"
    [[ "$rows" -eq 5 ]] || {
        printf 'test-all: owners 必须恰好覆盖五个 suite，实际 %d\n' "$rows" >&2
        return 1
    }
    local required
    for required in "${OWNER_ORDER[@]}"; do
        [[ -v "OWNER_ROLE[$required]" ]] || {
            printf 'test-all: owners 缺少 suite：%s\n' "$required" >&2
            return 1
        }
    done
}

profile_defaults() {
    case "$PROFILE" in
        unit) printf '%s\n' 'schema server client tiandao' ;;
        contract) printf '%s\n' 'schema scripts' ;;
        full) printf '%s\n' 'schema server client tiandao scripts' ;;
        e2e|preview) printf '%s\n' scripts ;;
    esac
}

resolve_suites() {
    DEFAULTS="$(profile_defaults)"
    local suite
    declare -A requested=()
    if ((${#REQUESTED[@]} == 0)); then
        for suite in $DEFAULTS; do requested["$suite"]=1; done
    else
        for suite in "${REQUESTED[@]}"; do
            case "$suite" in
                server|client|schema|tiandao|scripts) ;;
                *) usage_error "未知 suite：$suite" ;;
            esac
            case "$PROFILE:$suite" in
                unit:server|unit:client|unit:schema|unit:tiandao|contract:schema|contract:scripts|full:server|full:client|full:schema|full:tiandao|full:scripts|e2e:scripts|preview:scripts) ;;
                *) usage_error "suite $suite 不属于 profile $PROFILE" ;;
            esac
            requested["$suite"]=1
        done
    fi
    SELECTED=()
    for suite in $DEFAULTS; do
        [[ -v "requested[$suite]" ]] && SELECTED+=("$suite")
    done
    ((${#SELECTED[@]} > 0)) || usage_error "没有可执行的 suite"
}

list_command() {
    case "$1:$PROFILE" in
        server:unit|server:full) printf '%s' 'server build-token cargo fmt/clippy/test（full 追加 cargo build）' ;;
        client:unit) printf '%s' 'client build-token gradle test（保留 runGametest 依赖）' ;;
        client:full) printf '%s' 'client build-token gradle test build（含 runGametest）' ;;
        schema:unit) printf '%s' 'agent npm run build/check/test -w @bong/schema' ;;
        schema:contract) printf '%s' 'agent npm run generate:check -w @bong/schema' ;;
        schema:full) printf '%s' 'schema build/check/test + generate:check' ;;
        tiandao:unit) printf '%s' 'agent/packages/tiandao npm test（含 tsc 前置）' ;;
        tiandao:full) printf '%s' 'tiandao npm test + npm run build' ;;
        scripts:unit|scripts:contract|scripts:full) printf '%s' 'scripts/tests + resourcepack/preview/asset validators（contract）' ;;
        scripts:e2e) printf '%s' 'smoke-test-e2e.sh + bot-e2e.sh + e2e-chat-signal-window.sh' ;;
        scripts:preview) printf '%s' 'run-server-headless + runClientPreview + validate/compose' ;;
        *) printf '%s' 'existing native commands' ;;
    esac
}

list_dependencies() {
    case "$1" in
        server) printf '%s' 'cargo rustc build-token' ;;
        client) printf '%s' 'Java 17 client/gradlew' ;;
        schema|tiandao) printf '%s' 'Node npm agent/node_modules' ;;
        scripts) printf '%s' 'Bash Python zip gcc numpy Pillow existing validator dependencies' ;;
    esac
}

list_reports() {
    case "$1" in
        server) printf '%s' 'server/target; .sisyphus/evidence/evidence-server-test' ;;
        client) printf '%s' 'client/build/test-results; client/build/reports; client/build/gametest-results.xml' ;;
        schema) printf '%s' 'agent/packages/schema/dist; agent/packages/schema/generated' ;;
        tiandao) printf '%s' 'agent/packages/tiandao/dist' ;;
        scripts) printf '%s' '.sisyphus/evidence; client/resourcepack; preview output' ;;
    esac
}

print_list() {
    load_owners || { printf 'test-all: --list owners 校验失败\n' >&2; exit 2; }
    printf 'suite\towner_role\treviewer_path\tevidence\tcommand\tdependencies\tnative_reports\n'
    local suite
    for suite in "${OWNER_ORDER[@]}"; do
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$suite" "${OWNER_ROLE[$suite]}" "${REVIEWER_PATH[$suite]}" "${EVIDENCE_PATH[$suite]}" \
            "$(list_command "$suite")" "$(list_dependencies "$suite")" "$(list_reports "$suite")"
    done
    exit 0
}

resolve_suites
((LIST == 1)) && print_list
load_owners || { printf 'test-all: owners 映射校验失败\n' >&2; exit 2; }

REPORT_DIR=
RUN_ID=
PREVIEW_HANDOFF_FAILURE_EXIT=75
STARTED_AT=
ENDED_AT=
GIT_SHA=unknown
REPORT_ERROR=0
OVERALL_FAILURE=0
PREFLIGHT_FAILURE=0
PRECHECK_STATUS=PASS
PRECHECK_REASON=
PREVIEW_RASTER_DIR=
PREVIEW_CLIENT_DIR=
PREVIEW_CONFIG=
JAVA17_HOME=

declare -a RECORD_SUITE=()
declare -a RECORD_STATUS=()
declare -a RECORD_EXIT=()
declare -a RECORD_OWNER=()
declare -a RECORD_CWD=()
declare -a RECORD_COMMAND=()
declare -a RECORD_REASON=()
declare -a RECORD_ARTIFACTS=()
RECORD_COUNT=0

if [[ -n "$REPORT_ARG" ]]; then
    [[ "$REPORT_ARG" != "-" ]] || usage_error "--report-dir 不能是 -"
    if [[ "$REPORT_ARG" == /* ]]; then REPORT_DIR="$REPORT_ARG"; else REPORT_DIR="$CALLER_PWD/$REPORT_ARG"; fi
else
    RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
    REPORT_DIR="$ROOT/.sisyphus/evidence/test-all/$RUN_ID"
fi
if [[ -e "$REPORT_DIR" && -L "$REPORT_DIR" ]]; then
    printf 'test-all: report 目录不得是符号链接：%s\n' "$REPORT_DIR" >&2
    exit 3
fi
if ! mkdir -p -- "$REPORT_DIR" || [[ ! -d "$REPORT_DIR" || ! -w "$REPORT_DIR" ]]; then
    printf 'test-all: 无法创建或写入 report 目录：%s\n' "$REPORT_DIR" >&2
    exit 3
fi
[[ -n "$RUN_ID" ]] || RUN_ID="$(basename -- "$REPORT_DIR")"
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if GIT_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null)"; then :; else GIT_SHA=unknown; fi

json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    value="${value//$'\t'/\\t}"
    printf '%s' "$value"
}

append_record() {
    local suite="$1" status="$2" exit_code="$3" cwd="$4"
    local command_text="$5" reason="$6" artifacts="$7" initialize="${8:-1}"
    local suite_dir="$REPORT_DIR/$suite"
    RECORD_SUITE+=("$suite")
    RECORD_STATUS+=("$status")
    RECORD_EXIT+=("$exit_code")
    RECORD_OWNER+=("${OWNER_ROLE[$suite]}")
    RECORD_CWD+=("$cwd")
    RECORD_COMMAND+=("$command_text")
    RECORD_REASON+=("$reason")
    RECORD_ARTIFACTS+=("$artifacts")
    RECORD_COUNT=$((RECORD_COUNT + 1))
    [[ "$initialize" -eq 1 ]] || return 0
    if ! mkdir -p -- "$suite_dir"; then REPORT_ERROR=1; return 0; fi
    printf '%s\n' "$command_text" > "$suite_dir/command.txt" || REPORT_ERROR=1
    printf '%s\n' "$status" > "$suite_dir/status" || REPORT_ERROR=1
    : > "$suite_dir/stdout.log" || REPORT_ERROR=1
    printf '%s\n' "$reason" > "$suite_dir/stderr.log" || REPORT_ERROR=1
}

record_skip_or_blocked() {
    local suite="$1" status="$2" reason="$3" command_text="$4" cwd="$5" artifacts="$6"
    append_record "$suite" "$status" 125 "$cwd" "$command_text" "$reason" "$artifacts"
    OVERALL_FAILURE=1
}

need_command() { command -v "$1" >/dev/null 2>&1; }

java_major() {
    local java_bin="$1" first_line="" major=""
    first_line="$({ "$java_bin" -version 2>&1 || true; } | sed -n '1p')"
    major="$(printf '%s\n' "$first_line" | sed -nE 's/.*version "([0-9]+).*/\1/p; s/.*openjdk ([0-9]+).*/\1/p' | sed -n '1p')"
    printf '%s' "$major"
}

resolve_java17() {
    local candidates=() candidate="" major="" java_on_path=""
    if [[ -n "${JAVA_HOME:-}" && -x "${JAVA_HOME}/bin/java" ]]; then candidates+=("${JAVA_HOME}/bin/java"); fi
    candidates+=(/usr/lib/jvm/java-17-openjdk-amd64/bin/java /usr/lib/jvm/java-17-openjdk/bin/java /usr/lib/jvm/temurin-17-jdk-amd64/bin/java)
    java_on_path="$(command -v java 2>/dev/null || true)"
    [[ -n "$java_on_path" ]] && candidates+=("$java_on_path")
    for candidate in "${candidates[@]}"; do
        [[ -x "$candidate" ]] || continue
        major="$(java_major "$candidate")"
        if [[ "$major" == 17 ]]; then
            JAVA17_HOME="$(cd -- "$(dirname -- "$(readlink -f -- "$candidate" 2>/dev/null || printf '%s' "$candidate")")/.." && pwd -P)"
            return 0
        fi
    done
    return 1
}

resolve_preview_inputs() {
    local raw="" manifest="" client_dir="${BONG_CLIENT_PREVIEW_DIR:-}"
    local config="${BONG_PREVIEW_CONFIG:-$ROOT/client/preview-harness.json}"
    if [[ -n "${BONG_TERRAIN_RASTER_DIR:-}" ]]; then
        raw="$BONG_TERRAIN_RASTER_DIR"
        [[ -d "$raw" ]] || { PRECHECK_STATUS=BLOCKED; PRECHECK_REASON="BONG_TERRAIN_RASTER_DIR 不存在：$raw"; return 1; }
        PREVIEW_RASTER_DIR="$(cd -- "$raw" && pwd -P)" || return 1
    elif [[ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]]; then
        manifest="$BONG_TERRAIN_RASTER_PATH"
        [[ -f "$manifest" ]] || { PRECHECK_STATUS=BLOCKED; PRECHECK_REASON="BONG_TERRAIN_RASTER_PATH 不是可读 manifest：$manifest"; return 1; }
        PREVIEW_RASTER_DIR="$(cd -- "$(dirname -- "$manifest")" && pwd -P)" || return 1
    else
        PRECHECK_STATUS=BLOCKED
        PRECHECK_REASON='缺少 BONG_TERRAIN_RASTER_DIR 或 BONG_TERRAIN_RASTER_PATH（外部 raster handoff）'
        return 1
    fi
    local raster_file
    for raster_file in focus-layout-preview.png focus-surface-preview.png; do
        [[ -f "$PREVIEW_RASTER_DIR/$raster_file" ]] || {
            PRECHECK_STATUS=BLOCKED
            PRECHECK_REASON="外部 raster handoff 缺少只读输入：$PREVIEW_RASTER_DIR/$raster_file"
            return 1
        }
    done
    [[ -n "$client_dir" ]] || { PRECHECK_STATUS=BLOCKED; PRECHECK_REASON='preview 必须由调用方显式提供 BONG_CLIENT_PREVIEW_DIR'; return 1; }
    [[ "$client_dir" == /* ]] || client_dir="$CALLER_PWD/$client_dir"
    [[ -d "$client_dir" ]] || { PRECHECK_STATUS=BLOCKED; PRECHECK_REASON="BONG_CLIENT_PREVIEW_DIR 不存在：$client_dir"; return 1; }
    PREVIEW_CLIENT_DIR="$(cd -- "$client_dir" && pwd -P)" || return 1
    [[ "$config" == /* ]] || config="$CALLER_PWD/$config"
    [[ -f "$config" ]] || { PRECHECK_STATUS=BLOCKED; PRECHECK_REASON="BONG_PREVIEW_CONFIG 不存在：$config"; return 1; }
    PREVIEW_CONFIG="$(cd -- "$(dirname -- "$config")" && pwd -P)/$(basename -- "$config")" || return 1
}

preflight_suite() {
    local suite="$1"
    PRECHECK_STATUS=PASS
    PRECHECK_REASON=
    case "$suite" in
        server)
            if ! need_command cargo || ! need_command rustc || [[ ! -x "$ROOT/scripts/build-token.sh" ]]; then
                PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 cargo/rustc 或 scripts/build-token.sh'
            fi
            ;;
        client)
            if [[ ! -x "$ROOT/client/gradlew" ]]; then
                PRECHECK_STATUS=SKIP; PRECHECK_REASON='client/gradlew 不存在或不可执行'
            elif ! resolve_java17; then
                PRECHECK_STATUS=SKIP
                if need_command java; then PRECHECK_REASON='需要 Java 17，当前 java 不是 Java 17'; else PRECHECK_REASON='缺少 Java 17'; fi
            fi
            ;;
        schema|tiandao)
            if ! need_command node || ! need_command npm; then
                PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 node/npm'
            elif [[ ! -d "$ROOT/agent/node_modules" ]]; then
                PRECHECK_STATUS=SKIP; PRECHECK_REASON='agent/node_modules 未安装；入口不代为 npm ci'
            fi
            ;;
        scripts)
            if ! need_command bash || ! need_command python3; then
                PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 bash/python3'
            elif [[ "$PROFILE" == preview ]]; then
                # 先判定外部 handoff；缺 raster 必须是 BLOCKED，即使本机随后还缺
                # Java/Cargo，也不能把“调用方没有输入”误报成普通工具 SKIP。
                resolve_preview_inputs || true
                if [[ "$PRECHECK_STATUS" != PASS ]]; then
                    :
                elif ! need_command cargo || ! need_command rustc || [[ ! -x "$ROOT/scripts/build-token.sh" ]]; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='preview 缺少 cargo/rustc 或 build-token.sh'
                elif [[ ! -r "$ROOT/scripts/preview/run-server-headless.sh" \
                    || ! -r "$ROOT/scripts/preview/stop-server-headless.sh" ]]; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='preview handoff wrapper 不存在或不可读'
                elif ! need_command xvfb-run; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='preview 需要 xvfb-run（无 DISPLAY 时的 headless X server）'
                elif [[ ! -x "$ROOT/client/gradlew" ]]; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='preview 缺少 client/gradlew'
                elif ! resolve_java17; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='preview 需要 Java 17'
                fi
            elif [[ "$PROFILE" == contract || "$PROFILE" == full ]]; then
                if [[ ! -d "$ROOT/scripts/tests" ]]; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='scripts/tests 不存在'
                elif ! need_command zip; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 zip（resourcepack contract 既有前置）'
                elif ! need_command gcc; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 gcc（preview lifecycle contract 既有前置）'
                elif ! python3 -c 'import numpy; import PIL' >/dev/null 2>&1; then
                    PRECHECK_STATUS=SKIP; PRECHECK_REASON='缺少 Python asset/model 依赖（numpy/Pillow）'
                fi
            elif [[ "$PROFILE" == e2e ]]; then
                local e2e_path=""
                for e2e_path in \
                    "$ROOT/scripts/smoke-test-e2e.sh" \
                    "$ROOT/scripts/bot-e2e.sh" \
                    "$ROOT/scripts/e2e-chat-signal-window.sh"; do
                    if [[ ! -r "$e2e_path" ]]; then
                        PRECHECK_STATUS=SKIP; PRECHECK_REASON="e2e script 不存在或不可读：$e2e_path"
                        break
                    fi
                done
            fi
            ;;
    esac
    [[ "$PRECHECK_STATUS" == PASS ]]
}

run_suite() {
    local suite="$1" command_text="$2" runner="$3" cwd="$4" artifacts="$5"
    local suite_dir="$REPORT_DIR/$suite" pipeline_status=()
    local command_status=0 tee_status=0

    if ! mkdir -p -- "$suite_dir"; then
        REPORT_ERROR=1
        append_record "$suite" FAIL 3 "$cwd" "$command_text" 'report 文件无法写入' "$artifacts"
        OVERALL_FAILURE=1
        return 3
    fi
    printf '%s\n' "$command_text" > "$suite_dir/command.txt" || REPORT_ERROR=1
    : > "$suite_dir/stdout.log" || REPORT_ERROR=1
    : > "$suite_dir/stderr.log" || REPORT_ERROR=1
    if ((REPORT_ERROR == 1)); then
        printf '%s\n' 'report 文件无法写入' > "$suite_dir/stderr.log" 2>/dev/null || true
        printf '%s\n' FAIL > "$suite_dir/status" 2>/dev/null || true
        append_record "$suite" FAIL 3 "$cwd" "$command_text" 'report 文件无法写入' "$artifacts" 0
        OVERALL_FAILURE=1
        return 3
    fi

    # 立即读取 PIPESTATUS[0]，不能让 tee 掩盖 native command 的退出码。
    "$runner" 2> "$suite_dir/stderr.log" | tee "$suite_dir/stdout.log" > /dev/null
    pipeline_status=("${PIPESTATUS[@]}")
    command_status="${pipeline_status[0]:-125}"
    tee_status="${pipeline_status[1]:-125}"
    if [[ "$tee_status" -ne 0 ]]; then command_status=3; REPORT_ERROR=1; fi
    if [[ "$command_status" -eq 0 ]]; then
        printf '%s\n' PASS > "$suite_dir/status" || REPORT_ERROR=1
        append_record "$suite" PASS 0 "$cwd" "$command_text" completed "$artifacts" 0
    else
        printf '%s\n' FAIL > "$suite_dir/status" || REPORT_ERROR=1
        append_record "$suite" FAIL "$command_status" "$cwd" "$command_text" 'native command failed; see stdout.log/stderr.log' "$artifacts" 0
        OVERALL_FAILURE=1
    fi
    return "$command_status"
}

run_server_unit() {
    (
        cd -- "$ROOT/server" || exit 1
        "$ROOT/scripts/build-token.sh" cargo fmt --check &&
            "$ROOT/scripts/build-token.sh" cargo clippy --all-targets -- -D warnings &&
            "$ROOT/scripts/build-token.sh" cargo test
    )
}
run_server_full() {
    (
        cd -- "$ROOT/server" || exit 1
        "$ROOT/scripts/build-token.sh" cargo fmt --check &&
            "$ROOT/scripts/build-token.sh" cargo clippy --all-targets -- -D warnings &&
            "$ROOT/scripts/build-token.sh" cargo test &&
            "$ROOT/scripts/build-token.sh" cargo build
    )
}
run_client_unit() {
    (
        export JAVA_HOME="$JAVA17_HOME"
        export PATH="$JAVA17_HOME/bin:$PATH"
        export GRADLE_USER_HOME="${GRADLE_USER_HOME:-/tmp/bong-gradle}"
        cd -- "$ROOT/client" || exit 1
        "$ROOT/scripts/build-token.sh" gradle test
    )
}
run_client_full() {
    (
        export JAVA_HOME="$JAVA17_HOME"
        export PATH="$JAVA17_HOME/bin:$PATH"
        export GRADLE_USER_HOME="${GRADLE_USER_HOME:-/tmp/bong-gradle}"
        cd -- "$ROOT/client" || exit 1
        "$ROOT/scripts/build-token.sh" gradle test build
    )
}
run_schema_unit() {
    (
        cd -- "$ROOT/agent" || exit 1
        npm run build -w @bong/schema &&
            npm run check -w @bong/schema &&
            npm test -w @bong/schema
    )
}
run_schema_contract() {
    (
        cd -- "$ROOT/agent" || exit 1
        npm run generate:check -w @bong/schema
    )
}
run_schema_full() {
    (
        cd -- "$ROOT/agent" || exit 1
        npm run build -w @bong/schema &&
            npm run check -w @bong/schema &&
            npm test -w @bong/schema &&
            npm run generate:check -w @bong/schema
    )
}
run_tiandao_unit() {
    (cd -- "$ROOT/agent/packages/tiandao" && npm test)
}
run_tiandao_full() {
    (cd -- "$ROOT/agent/packages/tiandao" && npm test && npm run build)
}

run_scripts_contract() {
    local failed=0 found=0 test_file="" base=""
    local required_path=""
    for required_path in \
        "$ROOT/scripts/tests" \
        "$ROOT/scripts/test_build_resourcepack.py" \
        "$ROOT/scripts/preview" \
        "$ROOT/scripts/images" \
        "$ROOT/modelScript/tests"; do
        if [[ ! -e "$required_path" ]]; then
            printf 'scripts contract: required validator path missing: %s\n' "$required_path" >&2
            failed=1
        fi
    done
    while IFS= read -r -d '' test_file; do
        base="$(basename -- "$test_file")"
        [[ "$base" == test_all_contract_test.sh ]] && continue
        found=$((found + 1))
        case "$test_file" in
            *.sh) bash "$test_file" || failed=1 ;;
            *.py) python3 "$test_file" || failed=1 ;;
        esac
    done < <(find "$ROOT/scripts/tests" -maxdepth 1 -type f \( -name '*.sh' -o -name '*.py' \) -print0 | sort -z)
    ((found > 0)) || { printf 'scripts contract: 未发现 scripts/tests 合同测试\n' >&2; failed=1; }
    (cd -- "$ROOT" && python3 -m unittest scripts/test_build_resourcepack.py) || failed=1
    (cd -- "$ROOT" && python3 -m unittest discover -s modelScript/tests -p 'test_*.py') || failed=1
    (cd -- "$ROOT/scripts/preview" && python3 -m unittest discover -s . -p 'test_*.py') || failed=1
    local image_test
    local image_found=0
    while IFS= read -r -d '' image_test; do
        image_found=1
        python3 "$image_test" || failed=1
    done < <(find "$ROOT/scripts/images" -maxdepth 1 -type f -name 'test_*.py' -print0 | sort -z)
    ((image_found == 1)) || { printf 'scripts contract: 未发现 scripts/images asset tests\n' >&2; failed=1; }
    return "$failed"
}

run_e2e_scripts() {
    local failed=0 e2e_script=""
    for e2e_script in "$ROOT/scripts/smoke-test-e2e.sh" "$ROOT/scripts/bot-e2e.sh" "$ROOT/scripts/e2e-chat-signal-window.sh"; do
        if [[ ! -r "$e2e_script" ]]; then
            printf 'e2e script 不存在或不可读：%s\n' "$e2e_script" >&2
            failed=1
            continue
        fi
        bash "$e2e_script" || failed=1
    done
    return "$failed"
}

run_preview_handoff() {
    local failed=0
    # run_suite executes this runner on the left side of a pipeline.  These
    # cleanup variables must therefore survive the function return until the
    # pipeline subshell fires its EXIT trap; making them local loses ownership
    # exactly on the retry path.
    server_started=0
    preview_launcher_pid=0
    preview_exit_cleanup_active=0
    preview_launch_ready=0
    preview_launcher_status=0
    preview_launch_state_file="$REPORT_DIR/.preview-launch-${BASHPID}.state"
    if [[ -e "$preview_launch_state_file" || -L "$preview_launch_state_file" ]]; then
        printf 'preview launch state 已存在，拒绝复用：%s\n' "$preview_launch_state_file" >&2
        return 1
    fi
    preview_launch_state_published() {
        [[ -f "$preview_launch_state_file" && ! -L "$preview_launch_state_file" ]] \
            && grep -Fxq 'state=authority_published' "$preview_launch_state_file"
    }
    preview_authority_record_present() {
        # Presence only grants a cleanup attempt. stop-server-headless.sh is
        # still the authority boundary: it validates PID/starttime/executable
        # identity before sending any signal or removing the record.
        [[ -f "$REPORT_DIR/preview-server.pid" && ! -L "$REPORT_DIR/preview-server.pid" ]]
    }
    preview_clear_launch_state() {
        if [[ ! -e "$preview_launch_state_file" && ! -L "$preview_launch_state_file" ]]; then
            return 0
        fi
        if [[ -L "$preview_launch_state_file" || ! -f "$preview_launch_state_file" ]]; then
            printf 'preview cleanup: launch state 不是安全普通文件，保留现场：%s\n' \
                "$preview_launch_state_file" >&2
            return 1
        fi
        rm -f -- "$preview_launch_state_file"
    }
    preview_cleanup() {
        local exit_code="$1"
        if ((server_started == 0)) && preview_launch_state_published; then
            # The wrapper may have returned non-zero after publishing its
            # authority.  The marker is the explicit ownership handoff.
            server_started=1
        fi
        if ((server_started == 1)); then
            if BONG_PREVIEW_PID_FILE="$REPORT_DIR/preview-server.pid" \
                bash "$ROOT/scripts/preview/stop-server-headless.sh"; then
                server_started=0
            else
                printf 'preview cleanup: stop-server-headless.sh 未确认成功；保留清理状态\n' >&2
                exit_code=1
            fi
        fi
        if ((server_started == 0)) && preview_clear_launch_state; then
            trap - EXIT INT TERM
        elif ((server_started == 0)); then
            exit_code=1
        fi
        return "$exit_code"
    }
    preview_shutdown_launcher() {
        local signal_name="$1" launcher_status=0
        if ((preview_launcher_pid > 1)); then
            kill "-$signal_name" "$preview_launcher_pid" 2>/dev/null || true
            if wait "$preview_launcher_pid"; then
                server_started=1
                preview_launch_ready=1
            else
                launcher_status=$?
            fi
            preview_launcher_pid=0
            if preview_launch_state_published; then
                # A non-zero wrapper exit can still own the server record when
                # its internal rollback was not confirmed.  Keep the outer
                # stop retry eligible, but never run the client against a
                # launch that did not return ready.
                server_started=1
            elif ((launcher_status == PREVIEW_HANDOFF_FAILURE_EXIT)) \
                && preview_authority_record_present; then
                # A missing marker is eligible only for the wrapper's
                # handoff-publication failure code. Ordinary refusal (for
                # example, a reused report-dir with another server record)
                # must never turn pathname presence into ownership.
                server_started=1
            fi
            if ((launcher_status != 0)); then
                printf 'preview cleanup: run-server-headless.sh 在启动阶段以 exit %d 结束\n' \
                    "$launcher_status" >&2
            fi
        fi
    }
    preview_exit_cleanup() {
        local exit_code=$?
        if ((preview_exit_cleanup_active == 1)); then
            trap - EXIT INT TERM
            exit "$exit_code"
        fi
        preview_exit_cleanup_active=1
        preview_shutdown_launcher TERM
        preview_cleanup "$exit_code"
        exit_code=$?
        exit "$exit_code"
    }
    preview_signal_cleanup() {
        local exit_code="$1"
        if [[ "$exit_code" -eq 130 ]]; then
            preview_shutdown_launcher INT
        else
            preview_shutdown_launcher TERM
        fi
        preview_cleanup "$exit_code"
        exit_code=$?
        exit "$exit_code"
    }
    export BONG_PREVIEW_MODE=1
    export BONG_PREVIEW_CONFIG="$PREVIEW_CONFIG"
    export BONG_PREVIEW_SERVER=127.0.0.1:25565
    export BONG_PREVIEW_PID_FILE="$REPORT_DIR/preview-server.pid"
    export BONG_PREVIEW_LOG_FILE="$REPORT_DIR/preview-server.log"
    export BONG_PREVIEW_LAUNCH_STATE_FILE="$preview_launch_state_file"
    # Install cleanup before launch. run-server-headless owns the pre-publication
    # launch rollback; this outer trap only stops a server after that wrapper has
    # returned success and this process has confirmed server_started=1. A PID
    # pathname alone is never treated as ownership during startup cancellation.
    trap preview_exit_cleanup EXIT
    trap 'preview_signal_cleanup 130' INT
    trap 'preview_signal_cleanup 143' TERM
    bash "$ROOT/scripts/preview/run-server-headless.sh" --debug &
    preview_launcher_pid=$!
    if wait "$preview_launcher_pid"; then
        server_started=1
        preview_launch_ready=1
    else
        preview_launcher_status=$?
        failed=1
        if ((preview_launcher_status == PREVIEW_HANDOFF_FAILURE_EXIT)) \
            && preview_authority_record_present; then
            # The normal wait path does not pass through
            # preview_shutdown_launcher; preserve the same dedicated-code
            # ownership gate here.
            server_started=1
        fi
    fi
    preview_launcher_pid=0
    if ((server_started == 1 && preview_launch_ready == 1)); then
        (
            export JAVA_HOME="$JAVA17_HOME"
            export PATH="$JAVA17_HOME/bin:$PATH"
            export GRADLE_USER_HOME="${GRADLE_USER_HOME:-/tmp/bong-gradle}"
            cd -- "$ROOT/client" || exit 1
            xvfb-run -a --server-args='-screen 0 1280x720x24' \
                "$ROOT/scripts/build-token.sh" gradle runClientPreview
        ) || failed=1
    fi
    python3 "$ROOT/scripts/preview/validate_snapshots.py" --client-dir "$PREVIEW_CLIENT_DIR" --require-min-count 5 || failed=1
    python3 "$ROOT/scripts/preview/compose_grid.py" --client-dir "$PREVIEW_CLIENT_DIR" --raster-dir "$PREVIEW_RASTER_DIR" --out "$REPORT_DIR/preview-grid.png" || failed=1
    preview_cleanup 0 || failed=1
    return "$failed"
}

runner_for_suite() {
    case "$PROFILE:$1" in
        unit:server) printf '%s' run_server_unit ;;
        unit:client) printf '%s' run_client_unit ;;
        unit:schema) printf '%s' run_schema_unit ;;
        unit:tiandao) printf '%s' run_tiandao_unit ;;
        contract:schema) printf '%s' run_schema_contract ;;
        contract:scripts) printf '%s' run_scripts_contract ;;
        full:server) printf '%s' run_server_full ;;
        full:client) printf '%s' run_client_full ;;
        full:schema) printf '%s' run_schema_full ;;
        full:tiandao) printf '%s' run_tiandao_full ;;
        full:scripts) printf '%s' run_scripts_contract ;;
        e2e:scripts) printf '%s' run_e2e_scripts ;;
        preview:scripts) printf '%s' run_preview_handoff ;;
    esac
}
suite_cwd() {
    case "$1" in
        server) printf '%s' "$ROOT/server" ;;
        client) printf '%s' "$ROOT/client" ;;
        schema) printf '%s' "$ROOT/agent" ;;
        tiandao) printf '%s' "$ROOT/agent/packages/tiandao" ;;
        scripts) printf '%s' "$ROOT" ;;
    esac
}
suite_artifacts() {
    case "$1" in
        server) printf '%s' 'server/target|.sisyphus/evidence/evidence-server-test' ;;
        client) printf '%s' 'client/build/test-results/test|client/build/reports/tests/test|client/build/gametest-results.xml' ;;
        schema) printf '%s' 'agent/packages/schema/dist|agent/packages/schema/generated' ;;
        tiandao) printf '%s' 'agent/packages/tiandao/dist' ;;
        scripts)
            if [[ "$PROFILE" == preview ]]; then
                printf 'client preview screenshots|%s/preview-grid.png|%s/preview-server.log' "$REPORT_DIR" "$REPORT_DIR"
            else
                printf '%s' '.sisyphus/evidence|client/resourcepack|scripts validator stdout'
            fi
            ;;
    esac
}
suite_command() {
    case "$PROFILE:$1" in
        unit:server) printf '%s' 'cd server && build-token cargo fmt --check && build-token cargo clippy --all-targets -- -D warnings && build-token cargo test' ;;
        unit:client) printf '%s' 'cd client && build-token gradle test（保留 runGametest 依赖）' ;;
        unit:schema) printf '%s' 'cd agent && npm run build/check/test -w @bong/schema' ;;
        unit:tiandao) printf '%s' 'cd agent/packages/tiandao && npm test（含 tsc 前置）' ;;
        contract:schema) printf '%s' 'cd agent && npm run generate:check -w @bong/schema' ;;
        contract:scripts) printf '%s' 'scripts/tests 合同 + resourcepack/preview/asset validators' ;;
        full:server) printf '%s' 'unit server commands + build-token cargo build' ;;
        full:client) printf '%s' 'cd client && build-token gradle test build（含 runGametest）' ;;
        full:schema) printf '%s' 'schema build/check/test + generate:check' ;;
        full:tiandao) printf '%s' 'cd agent/packages/tiandao && npm test && npm run build' ;;
        full:scripts) printf '%s' 'scripts contract + resourcepack/preview/asset validators' ;;
        e2e:scripts) printf '%s' 'bash smoke-test-e2e.sh; bash bot-e2e.sh; bash e2e-chat-signal-window.sh' ;;
        preview:scripts) printf '%s' 'BONG_PREVIEW_MODE=1 run-server-headless + gradle runClientPreview + validate/compose' ;;
    esac
}

write_summary() {
    local i=0 artifact="" first=1
    local values=()
    ENDED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    if ! {
        printf '{\n'
        printf '  "run_id": "%s",\n' "$(json_escape "$RUN_ID")"
        printf '  "profile": "%s",\n' "$(json_escape "$PROFILE")"
        printf '  "git_sha": "%s",\n' "$(json_escape "$GIT_SHA")"
        printf '  "started_at": "%s",\n' "$(json_escape "$STARTED_AT")"
        printf '  "ended_at": "%s",\n' "$(json_escape "$ENDED_AT")"
        printf '  "report_dir": "%s",\n' "$(json_escape "$REPORT_DIR")"
        printf '  "suites": [\n'
        for ((i=0; i<RECORD_COUNT; i++)); do
            printf '    {"suite":"%s","status":"%s","exit_code":%s,"owner":"%s","cwd":"%s","command":"%s","reason":"%s","native_artifacts":[' \
                "$(json_escape "${RECORD_SUITE[$i]}")" \
                "$(json_escape "${RECORD_STATUS[$i]}")" \
                "${RECORD_EXIT[$i]}" \
                "$(json_escape "${RECORD_OWNER[$i]}")" \
                "$(json_escape "${RECORD_CWD[$i]}")" \
                "$(json_escape "${RECORD_COMMAND[$i]}")" \
                "$(json_escape "${RECORD_REASON[$i]}")"
            values=()
            if [[ -n "${RECORD_ARTIFACTS[$i]}" ]]; then IFS='|' read -r -a values <<< "${RECORD_ARTIFACTS[$i]}"; fi
            first=1
            for artifact in "${values[@]}"; do
                ((first == 1)) || printf ','
                printf '"%s"' "$(json_escape "$artifact")"
                first=0
            done
            printf ']}'
            ((i + 1 < RECORD_COUNT)) && printf ','
            printf '\n'
        done
        printf '  ]\n}\n'
    } > "$REPORT_DIR/summary.json"; then REPORT_ERROR=1; fi

    if ! {
        printf 'suite\tstatus\texit_code\towner\tcwd\tcommand\treason\tnative_artifacts\n'
        for ((i=0; i<RECORD_COUNT; i++)); do
            printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "${RECORD_SUITE[$i]}" "${RECORD_STATUS[$i]}" "${RECORD_EXIT[$i]}" \
                "${RECORD_OWNER[$i]}" "${RECORD_CWD[$i]}" "${RECORD_COMMAND[$i]}" \
                "${RECORD_REASON[$i]}" "${RECORD_ARTIFACTS[$i]}"
        done
    } > "$REPORT_DIR/summary.tsv"; then REPORT_ERROR=1; fi
}

previous_failed_suite=
for suite in "${SELECTED[@]}"; do
    command_text="$(suite_command "$suite")"
    runner="$(runner_for_suite "$suite")"
    cwd="$(suite_cwd "$suite")"
    artifacts="$(suite_artifacts "$suite")"
    if [[ -n "$previous_failed_suite" && "$CONTINUE" -eq 0 ]]; then
        record_skip_or_blocked "$suite" SKIP "前序 suite $previous_failed_suite 失败，未指定 --continue" "$command_text" "$cwd" "$artifacts"
        continue
    fi
    if ! preflight_suite "$suite"; then
        PREFLIGHT_FAILURE=1
        record_skip_or_blocked "$suite" "$PRECHECK_STATUS" "$PRECHECK_REASON" "$command_text" "$cwd" "$artifacts"
        previous_failed_suite="$suite"
        continue
    fi
    run_suite "$suite" "$command_text" "$runner" "$cwd" "$artifacts" || true
    last_index=$((RECORD_COUNT - 1))
    if [[ "${RECORD_STATUS[$last_index]}" != PASS ]]; then previous_failed_suite="$suite"; fi
done

write_summary
if ((REPORT_ERROR == 1)); then
    printf 'test-all: report 写入或 capture 失败，见：%s\n' "$REPORT_DIR" >&2
    exit 3
fi
printf '[test-all] profile=%s report=%s\n' "$PROFILE" "$REPORT_DIR"
if ((PREFLIGHT_FAILURE == 1)); then
    printf '[test-all] BLOCKED/SKIP: suite 前置条件未满足；详见 summary.json/summary.tsv\n' >&2
    exit 2
fi
if ((OVERALL_FAILURE == 0)); then
    printf '[test-all] PASS: %d suite(s)\n' "$RECORD_COUNT"
    exit 0
fi
printf '[test-all] FAIL: 至少一个 suite 非 PASS；详见 summary.json/summary.tsv\n' >&2
exit 1
