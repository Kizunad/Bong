"""灵晶（LingJing）—— 晶洞方块与悬浮双锥八面体灵核（纯材料级）共享色板。

【世界观与物理特质】：
- 物品来源：`ling_jing.png`
  “青云/血谷产，晶莹透亮法宝阵眼核心。”
- 彻底摒弃厚重石底座！
  灵晶是高纯度天地真元自然凝聚的高能晶簇，形态为悬浮的双锥梭形晶核（Double-terminated Crystal），
  内部封存着天然天道灵纹，伴生几枚微细的悬浮碎晶砾。
- 对应方块：
  深岩晶洞（Geode Block）—— 外部包裹深灰围岩，中心向内深陷破裂，露出内部幽蓝剔透的天然晶簇面。
"""

# 灵晶晶体与阵核（Spirit Crystal Body）
CRYSTAL_VOID   = (32, 36, 52)     # 深蓝包体阴影
CRYSTAL_DEEP   = (58, 72, 98)     # 深沉苍蓝晶核
CRYSTAL_MID    = (98, 126, 162)   # 幽蓝半透明晶身
CRYSTAL_LIT    = (148, 184, 222)  # 透光天蓝晶面
CRYSTAL_HIGH   = (212, 236, 255)  # 极亮蓝白高光与天道灵符

# 晶洞围岩（Geode Matrix Host Rock）
ROCK_DARK      = (30, 32, 36)     # 晶洞外层深灰玄岩
ROCK_BASE      = (50, 54, 60)     # 围岩基底
ROCK_LIT       = (76, 82, 90)     # 围岩受光面
