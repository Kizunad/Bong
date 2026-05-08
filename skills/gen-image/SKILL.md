---
name: gen-image
description: Bong 项目的图像生成。调 scripts/images/gen.py 按四档画风（item / particle / hud / scene）生成物品图标、粒子 VFX 贴图、HUD overlay、末法残土场景概念图。cliproxy 优先，失败 fallback openai。用法：/gen-image <style> <描述>，或直接说"生成一张 xxx 图/贴图/图标/场景"。
argument-hint: <style=item|particle|hud|scene> <物品或视觉描述>
allowed-tools: Read Write Edit Bash Glob Grep
---

# Bong 图像生成流程

你要替用户生图时走这套规范。生图入口统一在 `scripts/images/gen.py`，
画风约束在 `scripts/images/style.py`（三档常量）+ `local_images/generation_guide.md`
（细则文档）。

## 1. 触发条件

用户说到以下意图时走本 skill：

- "生成一张 XX 图标" / "画一个 XX 物品图" → **style=item**
- "生成一张 XX 粒子 / VFX 贴图" / "剑气 / 光柱 / 星屑 贴图" → **style=particle**
- "生成一张 HUD overlay / 水墨边框 / 结霜角 / 符阵贴图" → **style=hud**
- "生成一张 XX 场景 / 概念图 / 美学参考 / mood board / 立绘" / "末法残土 XX 的样子" → **style=scene**

不确定归哪档，**先问用户一句**，不要猜。

## 2. 四档画风对照

| style | 用途 | 背景默认 | 输出去处 | 后处理 |
|-------|------|---------|---------|--------|
| `item` | 物品图标（武器 / 药材 / 符牌 / 法宝） | `solid black / white / magenta` | `local_images/` 根 | `remove_bg.py` 抠纯色底 |
| `particle` | MC 粒子 / VFX 贴图（Line / Ribbon / Sprite / GroundDecal） | 纯黑 `#000000` | `local_images/particles/` | `lum_to_alpha.py` 亮度转 alpha |
| `hud` | HUD overlay（水墨边框 / 结霜 / 符阵） | `--transparent` 真 RGBA | `local_images/` 或 `client/.../textures/hud/` | 按需清白雾（RGB=0, alpha*=1-lum/255） |
| `scene` | 末法残土场景 / 概念图 / mood board（**非游戏资产**，美术对齐用） | 不透明完整画面 | `local_images/scenes/` | 无（直接看） |

**关键**：四档的 prefix 由 `--style` 自动拼接，**你不需要手写** `dark xianxia game item icon...`
这类 prefix 文案 —— 写描述文本即可，`gen.py` 会从 `style.py` 拼好。

## 3. 调用命令模板

### 3.1 物品图标

```bash
python scripts/images/gen.py \
  "a cluster of three hair-thin bone needles coated in corrupted spiritual residue, sickly green and black discoloration crawling along the tips" \
  --name 毒蛊飞针 \
  --style item \
  --save-prompt
```

**背景色决策**：
- 物品深色（骨器、铁器、黑石）：默认 `solid black background`，无需改
- 物品浅色（丝、白玉、白药）：手动在描述里替换成 `solid white background`
- 需要极精确抠图（毛发 / 细丝）：描述里写 `solid magenta background`

### 3.2 粒子 VFX 贴图

```bash
python scripts/images/gen.py \
  "a horizontal streak of concentrated sword qi, razor-thin filament of pure white light at the center spine, semi-translucent luminous haze feathering outward, left end solid hot white right end tapering into mist" \
  --name sword_qi_trail \
  --style particle \
  --transparent \
  --size 1536x1024 \
  --out local_images/particles/ \
  --save-prompt
```

生成完 **必须** 跑后处理：

```bash
python scripts/images/lum_to_alpha.py local_images/particles/sword_qi_trail.png
# → 得到 sword_qi_trail_alpha.png（RGBA, alpha=亮度, RGB=纯白可染色）
```

符文类（固定色不染色）加 `--no-tint`：

```bash
python scripts/images/lum_to_alpha.py local_images/particles/rune_char_dao.png --no-tint
```

**尺寸约定**（`local_images/generation_guide.md` §1.1）：
- Line（streak / beam） 128×32 或 256×32
- Ribbon（拖尾） 256×32，左右接缝无感
- GroundDecal（俯视圆环） 128×128 或 256×256
- Sprite（单体） 32×32 或 64×64
- 生成时用 `--size 1536x1024` 或 `1024x1024`，后续用 PIL 缩小

### 3.3 Scene / 概念图 / Mood Board

```bash
python scripts/images/gen.py \
  "a lone cultivator silhouette standing at the edge of a vast cracked rift valley at dusk, looking down into the collapsing void below, distant ash plumes rising on the horizon, withered jade-grey grass in the foreground, no celestial light, no flying sword, just emptiness and weight" \
  --name rift_valley_concept \
  --style scene \
  --size 1536x1024 \
  --out local_images/scenes/ \
  --save-prompt
```

**关键约束**：
- **不要 `--transparent`**：scene 是完整画面，不抠图、不接 alpha
- **横向构图**：默认 `--size 1536x1024`（电影画幅）；竖构图人物立绘用 `1024x1536`
- **palette 锁死**：prefix 已写死 `ash-grey and faded-jade`、`desaturated`、`oppressive overcast`、排除 `vibrant fantasy / celestial palace / pristine temple / anime / JRPG glow`。**别在 body 里破这套调子**——比如不要写 "vibrant red robe"、"glowing temple gates"
- **明确画里有什么**：地形特征（cracked stone / withered grass / bone-white strata / negative-pressure rift）+ 一个**焦点元素**（孤身修士剪影 / 干尸 / 灵龛入口 / 鲸落遗骸）+ 时间天气（dusk overcast / monsoon haze / winter thin snow）
- **避免常见 AI 套路**：明示 `no katana, no flying sword, no celestial dragon, no glowing palm strike, no anime character`，AI 一听到 "xianxia" 容易加这些

**用途**：美术对齐 / 概念图 / mood board，**不是**游戏内资产。生成完直接看，无后处理。

### 3.4 HUD Overlay

```bash
python scripts/images/gen.py \
  "four-corner Chinese ink wash (水墨) splashes, center 60% strictly fully transparent (alpha=0), irregular sumi-e brush strokes at corners only, soft bleeding edges, no rectangular frame" \
  --name ink_wash_vignette \
  --style hud \
  --transparent \
  --size 1536x1024 \
  --out local_images/particles/
```

**HUD 常见坑**：gpt-image 生的"透明 PNG" 边缘半透明像素 RGB 其实是白色（从白底抠），
叠到游戏场景上会产生白雾。生完必须清：

```python
from PIL import Image
import numpy as np
img = np.array(Image.open("xxx.png").convert("RGBA")).astype(np.float32)
rgb, alpha = img[:,:,:3], img[:,:,3]
lum = 0.299*rgb[:,:,0] + 0.587*rgb[:,:,1] + 0.114*rgb[:,:,2]
new_alpha = np.clip(alpha * (1.0 - lum / 255.0), 0, 255).astype(np.uint8)
out = np.zeros_like(img, dtype=np.uint8); out[:,:,3] = new_alpha
Image.fromarray(out).save("xxx.png")
```

## 4. 文件放置约定

- **生成物**：`local_images/`（整个目录在 `.gitignore`，不进 repo）
- **prompt 归档**：每次加 `--save-prompt`，写 `<name>_prompt.md` 到同目录
- **接入客户端**：手动把最终 PNG 拷到：
  - 粒子 → `client/src/main/resources/assets/bong/textures/particle/<name>.png`（去掉 `_alpha` 后缀；注意 namespace 是 `bong` 不是 `bong-client`）
  - HUD → `client/src/main/resources/assets/bong-client/textures/hud/<name>.png`
  - 物品 → `client/src/main/resources/assets/bong/textures/item/<name>/<variant>.png`
  - 场景 → 留在 `local_images/scenes/`，**不进客户端**（仅美术参考）

## 5. Backend

`gen.py --backend auto`（默认）先走 cliproxy；**只有**网络错误或空返回时 **自动 fallback openai**
（需要 `scripts/images/.env` 里有 `OPENAI_API_KEY`）。

强制某一端 `--backend cliproxy|openai`。

## 6. 完成后

- 给用户列出保存路径
- 如果是粒子 / HUD，提示下一步的后处理命令
- 如果图可能有画风偏移（AI 加了奇怪元素 / 背景不纯 / 颜色不对），**主动读一遍
  `local_images/generation_guide.md` §常见问题**再给出调整建议

## 7. 参考

- `scripts/images/README.md` — 工具链速览
- `scripts/images/style.py` — 三档 prefix 常量 source
- `local_images/generation_guide.md` — 画风细则（物品 + 粒子九档 + 符文）
