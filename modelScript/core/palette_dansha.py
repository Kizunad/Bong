"""丹砂（DanSha）方块材质与 3D 结晶模型共享色板。

色彩提取自原生图标 `dan_sha.png` 与末法红岩围岩基调。
"""

# 围岩（Host Rock）：暗褐/深红褐风化岩石
ROCK_DARK = (28, 14, 14)       # 最暗阴影岩缝
ROCK_BASE = (54, 24, 22)       # 围岩基底色
ROCK_MID = (78, 36, 32)        # 围岩表面漫射
ROCK_LIT = (108, 52, 44)       # 围岩受光粗粝边缘

# 丹砂晶体（Cinnabar Crystal）：朱红/血红高光六棱晶柱
CRYSTAL_DEEP = (124, 18, 16)   # 晶根/深层折射暗红
CRYSTAL_BODY = (186, 26, 22)   # 晶体饱满朱红主色
CRYSTAL_LIT = (226, 58, 42)    # 晶棱向阳透光朱红
CRYSTAL_HIGHLIGHT = (252, 148, 126) # 晶尖极亮漫射高光

# 渗出脉络（Cinnabar Vein）：岩缝渗出的炽红细脉
VEIN_GLOW = (218, 46, 30)      # 岩缝渗透微光
