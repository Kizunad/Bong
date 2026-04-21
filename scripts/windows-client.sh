#!/usr/bin/env bash
# Bong · windows-client.sh
#
# 把 client mod jar 同步到 Windows 侧的 HMCL 实例 mods 目录，可选自动打开启动器。
# 使用说明见 scripts/windows-client.md。

set -euo pipefail

SYNC_ONLY=false
LAUNCH_HMCL=false
for arg in "$@"; do
    case "$arg" in
        --sync-only)
            SYNC_ONLY=true
            ;;
        --launch|--open|--run)
            LAUNCH_HMCL=true
            ;;
        -h|--help)
            cat <<'USAGE'
Usage: bash scripts/windows-client.sh [--sync-only] [--launch]

默认：gradle build → 拷贝 client/build/libs/bong-client-*.jar 到 Windows 实例 mods
  --sync-only   只同步，不启动 HMCL（当前默认即此行为，flag 保留向后兼容）
  --launch      同步完成后自动打开 HMCL 启动器（D:\Minecraft\Open-Bong-HMCL.bat）
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

: "$SYNC_ONLY" # 保留变量避免 set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WIN_INSTANCE="/mnt/d/Minecraft/.minecraft/Fabric_Bang_Test"
MODS_DIR="$WIN_INSTANCE/mods"
HMCL_BAT_WIN='D:\Minecraft\Open-Bong-HMCL.bat'
HMCL_BAT_WSL='/mnt/d/Minecraft/Open-Bong-HMCL.bat'

if [[ ! -d "$WIN_INSTANCE" ]]; then
    echo "[windows-client] HMCL 实例目录不存在：$WIN_INSTANCE" >&2
    echo "                  先在 HMCL 里创建 Fabric_Bang_Test 实例（1.20.1 + Fabric Loader 0.16.10）" >&2
    exit 1
fi

echo "[windows-client] 构建 client（./gradlew build）..."
(cd "$ROOT/client" && ./gradlew build)

# Loom 产物：bong-client-<version>.jar（排除 sources / dev / javadoc）
JAR="$(ls -t "$ROOT"/client/build/libs/bong-client-*.jar 2>/dev/null \
        | grep -v -E '(-sources|-dev|-javadoc)\.jar$' \
        | head -n 1 || true)"

if [[ -z "$JAR" ]]; then
    echo "[windows-client] client/build/libs 下找不到 bong-client-*.jar（build 未产出？）" >&2
    exit 1
fi

mkdir -p "$MODS_DIR"
# 清掉旧的 bong-client-*.jar，避免 loader 同时加载多个版本
rm -f "$MODS_DIR"/bong-client-*.jar
cp -f "$JAR" "$MODS_DIR/"
echo "[windows-client] 已同步 $(basename "$JAR") → $MODS_DIR"

if [[ "$LAUNCH_HMCL" == true ]]; then
    if [[ ! -f "$HMCL_BAT_WSL" ]]; then
        echo "[windows-client] HMCL 启动器不存在：$HMCL_BAT_WIN" >&2
        exit 1
    fi
    if ! command -v cmd.exe >/dev/null 2>&1; then
        echo "[windows-client] 找不到 cmd.exe，非 WSL 环境？" >&2
        exit 1
    fi
    echo "[windows-client] 打开 HMCL：$HMCL_BAT_WIN"
    # 切到 /mnt/c 避免 cmd.exe 对 UNC/WSL 路径的警告；start "" 空标题分离新窗口
    (cd /mnt/c && cmd.exe /c start "" "$HMCL_BAT_WIN" >/dev/null 2>&1) || true
fi
