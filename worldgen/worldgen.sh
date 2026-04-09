#!/usr/bin/env bash
# 末法残土世界预生成脚本
# 用法: cd worldgen && bash worldgen.sh [radius_blocks]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVER_DIR="$SCRIPT_DIR/server"
DATAPACK_SRC="$SCRIPT_DIR/worldgen-mofa"
WORLD_NAME="mofa-world"
RADIUS="${1:-512}"  # 默认 512 格（32 chunks）

cd "$SERVER_DIR"

echo "=== 末法残土世界预生成 ==="
echo "半径: ${RADIUS} 格"
echo "服务端: $SERVER_DIR"

# 确保 datapack 已安装
mkdir -p "$WORLD_NAME/datapacks"
rm -rf "$WORLD_NAME/datapacks/worldgen-mofa"
cp -r "$DATAPACK_SRC" "$WORLD_NAME/datapacks/worldgen-mofa"
echo "[✓] Datapack 已安装"

# 确保 EULA
echo "eula=true" > eula.txt

# 用 FIFO 管道向服务端发命令
FIFO="/tmp/mc-worldgen-fifo-$$"
mkfifo "$FIFO"

echo "[...] 启动 Fabric 服务端"
java -Xmx2G -jar fabric-server-launch.jar --nogui < "$FIFO" &
SERVER_PID=$!

# 打开 FIFO 写端（防止 EOF 关闭）
exec 3>"$FIFO"

cleanup() {
    echo "清理中..."
    echo "stop" >&3 2>/dev/null || true
    exec 3>&- 2>/dev/null || true
    rm -f "$FIFO"
    wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

# 等待服务端就绪
echo "[...] 等待服务端启动完成"
for i in $(seq 1 120); do
    if [ -f "logs/latest.log" ] && grep -q "Done" "logs/latest.log" 2>/dev/null; then
        echo "[✓] 服务端已就绪 (${i}s)"
        break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "[✗] 服务端启动失败"
        cat logs/latest.log 2>/dev/null | tail -30
        exit 1
    fi
    sleep 1
done

# 检查 datapack 是否加载
sleep 2
echo "datapack list" >&3
sleep 2

# 开始 Chunky 预生成
echo "[...] 启动 Chunky 预生成 (半径 ${RADIUS})"
echo "chunky radius $RADIUS" >&3
sleep 1
echo "chunky start" >&3

# 等待 Chunky 完成
echo "[...] 等待预生成完成..."
DONE=false
for i in $(seq 1 600); do
    if grep -q "Task finished" "logs/latest.log" 2>/dev/null; then
        DONE=true
        echo "[✓] 预生成完成"
        break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "[✗] 服务端意外退出"
        exit 1
    fi
    # 每 30 秒打印进度
    if (( i % 30 == 0 )); then
        PROGRESS=$(grep -o "Generating.*%" "logs/latest.log" 2>/dev/null | tail -1 || echo "进行中...")
        echo "    [$((i))s] $PROGRESS"
    fi
    sleep 1
done

if [ "$DONE" = false ]; then
    echo "[!] 超时（600s），强制保存"
    echo "chunky cancel" >&3
    sleep 3
fi

# 保存并关闭
echo "save-all flush" >&3
sleep 5
echo "stop" >&3

# 等待服务端退出
wait "$SERVER_PID" 2>/dev/null || true

echo ""
echo "=== 生成完成 ==="
echo "世界目录: $SERVER_DIR/$WORLD_NAME/"
echo "Region 文件: $SERVER_DIR/$WORLD_NAME/region/"
ls -la "$WORLD_NAME/region/" 2>/dev/null || echo "(无 region 文件 — 检查日志)"
echo ""
echo "下一步: 将 $WORLD_NAME/ 目录路径配置给 Valence AnvilLevel"
