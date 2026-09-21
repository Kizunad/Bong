"""便携炼丹炉（AlchemyFurnace）共享色板。

色彩精确对应 `ref_alchemy_furnace_icon.png` 与 `ref_alchemy_furnace_exploded.png`：
- 玄石外壳（Dark Stone Shell）
- 黄铜锁箍与构件（Weathered Brass Fittings）
- 炉膛地火与炽炭（Hearth Fire Core）
"""

# 玄石炉身（Dark Volcanic Stone Shell）
STONE_VOID = (16, 16, 20)       # 极深阴影/背光
STONE_DARK = (32, 32, 38)       # 玄石深黑基底
STONE_MID  = (52, 54, 62)       # 玄石表面冷灰漫射
STONE_LIT  = (78, 80, 92)       # 倒角受光粗粝石面

# 黄铜锁箍、提环与排气阀（Weathered Brass Fittings）
BRASS_DARK = (68, 48, 26)       # 黄铜深色阴影/氧化缝
BRASS_BASE = (112, 78, 42)      # 斑驳古黄铜底色
BRASS_MID  = (148, 106, 56)     # 黄铜受光面
BRASS_LIT  = (188, 142, 76)     # 棱角明亮青铜金
BRASS_HIGH = (226, 184, 112)    # 金属倒角点状高光

# 炉膛地火核心（Hearth Fire Core）
FIRE_DEEP  = (148, 38, 16)      # 暗红炽热炭床底
FIRE_MID   = (226, 92, 24)      # 鲜艳炽火橙
FIRE_LIT   = (255, 172, 42)     # 金黄火舌
FIRE_CORE  = (255, 238, 140)    # 核心极亮白黄火苗
