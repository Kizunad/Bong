"""盐蓬晶（SaltCrystal）—— 盐晶岩方块与斜插深蓝透明盐晶簇（纯材料级）共享色板。

【原画与材料第一性原理深度还原】：
- 物品来源：`salt_crystal.png`（白盐蓬析出的硬质灵盐结晶，可诱鼠兼代灵石）
- 原画特征拆解：
  1. 正中斜向右上（~28°）挺拔刺出的巨型六方深海蓝半透明晶柱（Ocean Blue Oblique Pillar）。
  2. 晶体内部具有极其醒目的纵向“闪电白霜解理纹”（Lightning Frost Fracture），深蓝通透。
  3. 周围基座簇拥着层层咬合的深靛蓝与黑蓝色微缩次生晶簇（Deep Indigo Base Shards），如浪潮般托举主晶。
"""

# 深蓝黑/靛黑次生盐晶基质（Deep Indigo/Midnight Salt Base）
SALT_VOID   = (14, 16, 22)        # 晶根深暗阴影
SALT_DARK   = (28, 34, 48)        # 靛黑结晶底色
SALT_INDIGO = (46, 58, 82)        # 次生晶体漫射深蓝

# 主柱深海蓝半透明晶体（Ocean Blue Crystal Body）
CRYST_MID   = (72, 94, 134)       # 海蓝半透明晶身
CRYST_LIT   = (108, 142, 196)     # 晶棱透光蔚蓝
CRYST_HIGH  = (164, 202, 248)     # 晶尖向阳高光冷蓝

# 闪电白霜解理纹（Lightning Frost Fracture）
FROST_DEEP  = (130, 164, 220)     # 霜纹深层微光
FROST_WHITE = (226, 238, 255)     # 纯净白霜闪电裂隙（极亮冰白）
