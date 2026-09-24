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
| `review_icons.py` | 扫描 PNG 或读取前后对照清单，生成带快照的离线审图库 |

## 离线审图库

依赖 Python 3.11+ 与 Pillow。直接生成客户端物品图库：

```bash
python3 scripts/images/review_icons.py --out local_images/item-review

# 扫描其他 PNG 目录，按相对路径导出标记
python3 scripts/images/review_icons.py \
    --source local_images/new-icons --out local_images/new-icons-review

# 原图、生成输入、去背结果对照
python3 scripts/images/review_icons.py \
    --manifest local_images/remaster/review-data.json \
    --out local_images/remaster-review --title "道具重画与去背对照"
```

用浏览器打开输出目录里的 `index.html`，无需启动服务器。支持搜索、分类筛选、
深底/白底/黑底/棋盘底、32px/64px 实际尺寸预览，以及返工/重画/去背景标记和备注。
扫描物品目录时按文件名关联服务端 TOML 中的名称和原有描述；无对应定义时显示文件名。

对照清单结构如下，图片路径相对于清单文件，也可用绝对路径：

```json
{
  "images": [
    {
      "id": "I001",
      "name": "草绳",
      "path": "client/src/main/resources/assets/bong-client/textures/gui/items/grass_rope.png",
      "icon": "icons/grass_rope.png",
      "old": "originals/grass_rope.png",
      "input": "raw/grass_rope.png",
      "cutout": "cutouts/grass_rope.png",
      "mask": "masks/grass_rope.png",
      "redraw": true,
      "warnings": [],
      "note": "检查细绳边缘"
    }
  ]
}
```

`id`、`path`、`icon` 必填；编号和资源路径不可重复。其他字段可省略，
`remove_background: true` 用于归入原图去背景分类。只接受本地 PNG。
工具将输入图和缩略图复制到输出目录，保留 alpha 和长宽比，不修改输入资源。
原图快照应指向处理前的备份，以免客户端资源替换后失去对照。
整个输出目录可移动或分享；不覆盖已有输出目录，失败后重试请换一个新目录。

标记按图片内容与资源路径分批保存到浏览器，标题变化不会清空标记；图片变化会创建新批次。
导出文件为 `bong-icon-review-selection.json`，包含 `version`、`datasetId` 和 `selections`，
每项保留 `id`、`name`、`path`、`redo`、`redraw`、`remove_background`、`note`。
可在同批图库中导入，按资源路径合并；格式、批次或路径不匹配时整份拒绝，保留现有标记。
移动目录或换浏览器前先导出，浏览器本地保存不保证跨路径可用。

“不含透明像素”筛选只检查 alpha，不判断灰白棋盘是否被画进 RGB。
工具、HTML 模板进入 Git；图库、原图快照与导出清单保留在 `local_images/`。

## 画风细则

- 三档 prefix 的完整文案见 `style.py`。
- 物品图标的背景色取舍、修词、粒子九档专项 prompt、符文字符方案：
  统一看 `local_images/generation_guide.md`（这是 source of truth，别分裂）。

## 对外 skill

Claude Code agent 用 `/gen-image` skill（`.claude/skills/gen-image/SKILL.md`）
包装调用 `gen.py`，自动挑画风、处理后处理、提示用户文件位置。
