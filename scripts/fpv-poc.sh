#!/usr/bin/env bash
# plan-fpv-cast-av-v1 P0 —— FPV 技术路线 A/B/C POC 快捷启动。
#
# 用途：一条命令起 runClient，进游戏后用一个键实时切换第一人称路线（OFF/A/B/C），
# 施同一招（sword.cleave）逐路线肉眼对比【持物遮挡】——§8 #1 的决定性判据，只能真机看。
#
# 用法：
#   bash scripts/fpv-poc.sh            # 起 client（假设 server 已在跑）
#   bash scripts/fpv-poc.sh --help     # 只看操作步骤，不启动
#
# 前置：server 需先在另一个终端跑起来（本脚本不代管 server 生命周期）：
#   cd server && BONG_SKIP_SKIN_PREFETCH=1 cargo run      # 监听 :25565 offline
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

print_workflow() {
  cat <<'EOF'
──────────────────────────────────────────────────────────────────────────────
 FPV 路线 A/B/C POC —— 真机对比操作流
──────────────────────────────────────────────────────────────────────────────
 1) 起 server（另开终端，若未在跑）：
        cd server && BONG_SKIP_SKIN_PREFETCH=1 cargo run

 2) 本脚本起 client（Java 17 runClient / WSLg）。进游戏 → 多人 → 直连 localhost。

 3) 绑 FPV 切换键：选项 → 控制 → 「Bong 控制」→「FPV 路线切换（POC）」→ 绑一个键
    （默认不绑定，避免占 F1-F9 快捷栏；建议绑到 K 或小键盘）。

 4) 拿到 Bong 剑 + 会 sword.cleave（dev 命令，聊天栏）：
        /give bong:placeholder_sword          # 或对应剑模板；进 hotbar/装备
        /technique add sword_cleave            # 若未习得（视你存档而定）
    然后按你平时的施法方式放 sword.cleave（技能栏 / 快捷使用键）。

 5) 切到第一人称（默认视角，F5 循环回第一人称），反复：放 cleave → 按 FPV 键循环
    OFF → A → B → C → OFF（actionbar 会报当前路线）→ 再放 cleave，逐路线看：
      • 手臂动画有没有出现在主视角？
      • 【关键】持剑时：剑与手臂的遮挡对不对？有没有 vanilla 手/剑双重渲染、
        z-fighting、剑穿手、或剑被盖住？
      • body 位移有没有晃相机（cleave 有 body.z 前冲）？
      • 每条路线截图（F2，存 .minecraft/screenshots）。

 路线含义：
   OFF = 出厂现状（THIRD_PERSON_MODEL，第一人称只见持物、无手臂动画）
   A   = 库原生：THIRD_PERSON_MODEL + FirstPersonConfiguration 全开（手臂+持物）  ← 预判最优
   B   = 自绘层占位（NONE；自绘渲染器 P0 待补，暂等价 vanilla FP 手臂）
   C   = vanilla 注入占位（VANILLA；骨骼注入 mixin P0 待补，暂为库 vanilla FP）

 把 A（及需要时 B/C）的截图发我 → 我据【持物遮挡】收口 §8 #1、再往下 P1。
 说明：render_animation.py --fpv 只能 headless 迭代姿态，遮挡这条只认真机截图。
──────────────────────────────────────────────────────────────────────────────
EOF
}

print_workflow

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  exit 0
fi

# Java 17 for Fabric（系统默认常是 21）。优先用 sdkman 的 17，其次 JAVA17_HOME。
JAVA17="$(ls -d "$HOME"/.sdkman/candidates/java/17* 2>/dev/null | head -1 || true)"
if [[ -n "${JAVA17_HOME:-}" ]]; then
  export JAVA_HOME="$JAVA17_HOME"
elif [[ -n "$JAVA17" ]]; then
  export JAVA_HOME="$JAVA17"
fi
echo ">> JAVA_HOME=${JAVA_HOME:-<system default>}"
echo ">> 启动 runClient（Ctrl-C 退出）..."
cd "$REPO_ROOT/client"
exec ./gradlew runClient
