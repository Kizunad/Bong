#!/usr/bin/env bash
# Bong · windows-client.sh
#
# 把 client mod jar 同步到 Windows 侧的 HMCL 实例 mods 目录。
# 使用说明见 scripts/windows-client.md。

set -euo pipefail

SYNC_ONLY=false
for arg in "$@"; do
    case "$arg" in
        --sync-only)
            SYNC_ONLY=true
            ;;
        -h|--help)
            cat <<'USAGE'
Usage: bash scripts/windows-client.sh [--sync-only]

默认：gradle build → 拷贝 client/build/libs/bong-client-*.jar 到 Windows 实例 mods
  --sync-only   当前 MVP 与默认等价；flag 保留给未来区分 "build+sync" vs "build+runClient"
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

# SYNC_ONLY 当前为 future-proof flag（build+sync 是本脚本唯一行为），保留变量避免 set -u 报错
: "$SYNC_ONLY"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WIN_INSTANCE="/mnt/d/Minecraft/.minecraft/Fabric_Bang_Test"
MODS_DIR="$WIN_INSTANCE/mods"

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
