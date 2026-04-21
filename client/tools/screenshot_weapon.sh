#!/usr/bin/env bash
# Automated weapon screenshot via runClient + WeaponScreenshotHarness.
#
# 用法:
#   client/tools/screenshot_weapon.sh <asset_id> [vanilla_item_id]
#
# 示例:
#   client/tools/screenshot_weapon.sh placeholder_sword
#   client/tools/screenshot_weapon.sh cracked_heart minecraft:nether_star
#
# asset_id 对应 client/tools/asset_configs/<id>.json + client/src/main/resources/
# assets/bong/models/item/<id>/<id>.obj. vanilla_item_id 可省，脚本会扫
# assets/minecraft/models/item/*.json 找引用该 asset OBJ 的条目自动反推。
#
# 前置:
#   single-player 世界 bong_weapon_test 必须已存在（创造 + 允许作弊 + 超平坦）。
#   首次: 手动 `./gradlew runClient` → 单人 → 创建新世界，名字 "bong_weapon_test"，
#         其它随意，关游戏。之后本脚本用 --quickPlaySingleplayer 直进。
#
# 输出:
#   client/tools/renders/<asset_id>/mc_firstperson_righthand.png
#   client/tools/renders/<asset_id>/mc_thirdperson_righthand.png
#   client/tools/renders/<asset_id>/mc_hotbar.png

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$CLIENT_DIR/.." && pwd)"

if [ $# -lt 1 ]; then
    echo "用法: $0 <asset_id> [vanilla_item_id]" >&2
    exit 1
fi
asset_id="$1"
item_id="${2:-}"

# 反推 vanilla item id
if [ -z "$item_id" ]; then
    mc_item_dir="$CLIENT_DIR/src/main/resources/assets/minecraft/models/item"
    # 找一个 json 里 "model" 字段引用了 bong:models/item/<asset_id>/*.obj 的
    match=$(grep -l "bong:models/item/${asset_id}/" "$mc_item_dir"/*.json 2>/dev/null | head -1 || true)
    if [ -z "$match" ]; then
        echo "错误: 找不到引用 $asset_id OBJ 的 vanilla item override" >&2
        echo "  (在 $mc_item_dir/*.json 里 grep bong:models/item/${asset_id}/)" >&2
        echo "  请作为第二个参数显式传 item id, 如 minecraft:iron_sword" >&2
        exit 2
    fi
    item_name=$(basename "$match" .json)
    item_id="minecraft:$item_name"
    echo "[screenshot_weapon] 反推 item id: $item_id (来自 $(basename "$match"))"
fi

# 世界存在性预检
save_dir="$CLIENT_DIR/run/saves/bong_weapon_test"
if [ ! -f "$save_dir/level.dat" ]; then
    cat <<EOF >&2
错误: 测试世界不存在: $save_dir

首次手动创建流程（只要做一次）:
  1. cd $CLIENT_DIR && ./gradlew runClient
  2. 单人游戏 → 创建新世界
     - 名字: bong_weapon_test
     - 游戏模式: 创造
     - 世界类型: 超平坦 (或默认也行，推荐超平坦快载入)
     - 更多选项 → 允许作弊: 开
  3. 创建并进入世界，保存退出
  4. 重新跑本脚本
EOF
    exit 3
fi

out_dir="$CLIENT_DIR/tools/renders/$asset_id"
mkdir -p "$out_dir"

echo "[screenshot_weapon] asset_id=$asset_id"
echo "[screenshot_weapon] item_id=$item_id"
echo "[screenshot_weapon] out=$out_dir"
echo "[screenshot_weapon] 启动 runClient (可能 30-60s 冷启动)..."

cd "$CLIENT_DIR"
export BONG_WEAPON_TEST_ITEM="$item_id"
export BONG_WEAPON_TEST_ASSET="$asset_id"
# 必须绝对路径——MC cwd 在 client/run/, 相对路径会落到 run/client/tools/renders
export BONG_WEAPON_TEST_OUT="$(realpath "$CLIENT_DIR/tools/renders")"

# --quickPlaySingleplayer 直接进世界，跳标题页；harness 会自己 scheduleStop 退出
./gradlew runClient -x test --args="--quickPlaySingleplayer bong_weapon_test"

echo "[screenshot_weapon] 完成，产物:"
ls -la "$out_dir"/mc_*.png 2>/dev/null || {
    echo "  (没有 mc_*.png？harness 可能没跑通，检查 client/run/logs/latest.log)" >&2
    exit 4
}
