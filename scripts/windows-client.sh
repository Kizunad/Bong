#!/usr/bin/env bash
# Bong · windows-client.sh
#
# 把 client mod jar 同步到 Windows Native Fabric 实例的 mods 目录。
# 使用说明见 scripts/windows-client.md。

set -euo pipefail

for arg in "$@"; do
    case "$arg" in
        --sync-only)
            : # 与默认行为相同，保留旧调用兼容。
            ;;
        --launch|--open|--run)
            echo "[windows-client] 已移除 HMCL 启动入口；请使用 bong-native/forge-interact-launch.ps1 启动 Windows Native。" >&2
            exit 1
            ;;
        -h|--help)
            cat <<'USAGE'
Usage: bash scripts/windows-client.sh [--sync-only]

默认：Java 17 gradle build → 拷贝 client/build/libs/bong-client-*.jar 到 Windows Native 实例 mods
  --sync-only   只同步 jar（当前默认即此行为，保留参数兼容）
  --launch      已移除；请使用 bong-native/forge-interact-launch.ps1 启动原生客户端
  -h, --help    show this help

Windows 实例目录：D:\Minecraft\.minecraft\Fabric_Bang_Test (WSL: /mnt/d/...)
USAGE
            exit 0
            ;;
        *)
            echo "[windows-client] 未知参数：$arg" >&2
            exit 1
            ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JAVA17_HOME="${JAVA17_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
WIN_INSTANCE="/mnt/d/Minecraft/.minecraft/Fabric_Bang_Test"
MODS_DIR="$WIN_INSTANCE/mods"

if [[ ! -x "$JAVA17_HOME/bin/java" ]]; then
    echo "[windows-client] Java 17 不存在或不可执行：$JAVA17_HOME/bin/java" >&2
    echo "                  可用 JAVA17_HOME=/path/to/jdk17 覆盖" >&2
    exit 1
fi

if [[ ! -d "$WIN_INSTANCE" ]]; then
    echo "[windows-client] Windows Native 实例目录不存在：$WIN_INSTANCE" >&2
    echo "                  请先准备 Fabric_Bang_Test 实例（1.20.1 + Fabric Loader）" >&2
    exit 1
fi

if command -v powershell.exe >/dev/null 2>&1; then
    # NB: powershell.exe 经 WSL interop 调用时退出码可能被污染成 126（命令其实跑成功了）。
    #     这只是存活性安全检查，非零退出不该 abort 整个同步——用 `|| true` 兜住，
    #     真检测到 PID 仍会在下面的 [[ -n ]] 分支 abort。
    RUNNING_CLIENT_PID="$(
        powershell.exe -NoProfile -Command '$process = Get-CimInstance Win32_Process | Where-Object { $_.Name -match "^javaw?\.exe$" -and $_.CommandLine -match "Fabric_Bang_Test" } | Select-Object -First 1; if ($null -ne $process) { $process.ProcessId }' 2>/dev/null \
            | tr -d '\r' \
            | head -n 1 \
            || true
    )"
    if [[ -n "$RUNNING_CLIENT_PID" ]]; then
        echo "[windows-client] 检测到 Fabric_Bang_Test 客户端仍在运行（PID $RUNNING_CLIENT_PID）。" >&2
        echo "[windows-client] 请先关闭 Minecraft 客户端，再同步/启动；运行中覆盖 mod jar 会导致资源重载 ZipException。" >&2
        exit 1
    fi
fi

echo "[windows-client] 构建 client（构建令牌 wrapper gradle build）..."
(cd "$ROOT/client" && JAVA_HOME="$JAVA17_HOME" PATH="$JAVA17_HOME/bin:$PATH" "$ROOT/scripts/build-token.sh" gradle build)

# Loom 产物：bong-client-<version>.jar（排除 sources / dev / javadoc）
JAR="$(ls -t "$ROOT"/client/build/libs/bong-client-*.jar 2>/dev/null \
        | grep -v -E '(-sources|-dev|-javadoc)\.jar$' \
        | head -n 1 || true)"

if [[ -z "$JAR" ]]; then
    echo "[windows-client] client/build/libs 下找不到 bong-client-*.jar（build 未产出？）" >&2
    exit 1
fi

mkdir -p "$MODS_DIR"
# 清掉旧的同名 mod，避免 loader 同时加载多个版本
rm -f "$MODS_DIR"/bong-client-*.jar
cp -f "$JAR" "$MODS_DIR/"
echo "[windows-client] 已同步 $(basename "$JAR") → $MODS_DIR"
