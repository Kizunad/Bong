# scripts/images — Bong 图像生成工具链

Bong 所有美术资产（物品图标、VFX 粒子贴图、HUD overlay）走统一入口 `gen.py`
生成；后处理（抠图、亮度→alpha、预览）也在本目录。画风细则见
`local_images/generation_guide.md`。

## 快速上手

```bash
# 1. 配 env
cp scripts/images/.env.example scripts/images/.env
$EDITOR scripts/images/.env   # 至少填 OPENAI_API_KEY（fallback 用）

# 2. 生图
python scripts/images/gen.py "a cracked iron sword with glowing runes" \
    --name iron_sword --transparent                       # 物品图标（透明背景）

python scripts/images/gen.py "a horizontal streak of sword qi" \
    --name sword_qi_trail --style particle --transparent \
    --out local_images/particles/                          # 粒子贴图（黑底 + 后处理转 alpha）

python scripts/images/gen.py "four-corner sumi-e ink splashes, center transparent" \
    --name ink_wash_vignette --style hud --transparent --size 1536x1024   # HUD overlay
```

## 四档画风（`--style`）

| 档 | 用途 | 背景默认 | 后处理 |
|----|------|---------|--------|
| `item`（默认） | 物品图标（武器、药材、符牌…） | `--transparent` 真 alpha | 直接接入；旧纯色图可用 `remove_bg.py` 抠图 |
| `particle` | MC 粒子 / VFX 贴图 | 纯黑 `#000000` | `lum_to_alpha.py` 亮度转 alpha |
| `hud` | HUD overlay（水墨边框等） | `--transparent` 真 alpha | 走 `scripts/images/*.py` 清边缘白雾 |
| `scene` | 末法残土场景 / 概念图 / mood board（**非游戏资产**） | 不透明完整画面 | 无（直接看） |

`prefix` 直接嵌入 `style.py` 的常量，不再依赖 `prefix.md` 文件。

## Backend

- `--backend auto`（默认） — cliproxy 优先；网络错误 / 空返回时 **自动 fallback 到 openai**（需 `OPENAI_API_KEY`）。
- `--backend cliproxy` — 强制走自建 `/v1/responses` SSE。
- `--backend openai` — 强制走 `api.openai.com/v1/images/generations`。

## 本地产物

- 生成的 PNG 默认写到 `local_images/`（整个 `local_images/` 在 `.gitignore`，不进 repo）。
- 每次 `--save-prompt` 会同步写一份 `<name>_prompt.md` 归档到输出目录。
- 粒子贴图约定：**黑底原图** + `_alpha.png` 转透明版，两份并存；客户端只拷 `_alpha.png`（去掉后缀）到 `client/src/main/resources/assets/bong-client/textures/particle/`。

## 辅助脚本（本目录其他 `.py`）

| 脚本 | 作用 |
|------|------|
| `lum_to_alpha.py` | 黑底生成的粒子图 → RGBA alpha 通道；`--no-tint` 保原色 |
| `remove_bg.py` | 物品图 solid-color 背景抠透明 |
| `preview_particles.py` | 把 `local_images/particles/*.png` 拼成一张总览 |
| `preview_runes_grid.py` | 符文字符宫格预览 |
| `process_particles_batch.py` | 批量对粒子图跑 `lum_to_alpha` + 缩尺 |
| `render_rune_chars.py` | 用字体直接渲染符文汉字（不靠 AI 画中文） |

## 画风细则

- 三档 prefix 的完整文案见 `style.py`。
- 物品图标的背景色取舍、修词、粒子九档专项 prompt、符文字符方案：
  统一看 `local_images/generation_guide.md`（这是 source of truth，别分裂）。

## 对外 skill

Claude Code / opencode agent 用 `/gen-image` skill（`.claude/skills/gen-image/SKILL.md`）
包装调用 `gen.py`，自动挑画风、处理后处理、提示用户文件位置。
