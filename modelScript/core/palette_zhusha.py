"""朱砂（ZhuSha）方块材质与 3D 原矿模型共享色板。

色彩提取自原生图标 `zhu_sha.png` 与火山/血谷深层地质环境。
相比丹砂（红岩底+六棱晶），朱砂是通体如凝固血髓的整块多面体原矿，夹杂火山黑岩壳与熔岩裂纹。
"""

# 火山玄武岩外壳（Volcanic Crust）
CRUST_DARK = (24, 16, 18)        # 火山黑岩深阴影
CRUST_BASE = (46, 24, 28)        # 火山粗糙黑褐岩壳
CRUST_MID  = (68, 32, 36)        # 微风化岩皮

# 凝固血髓晶体（Blood Marrow Crystal Body）
BLOOD_DEEP = (98, 22, 28)        # 深层凝血暗红核
BLOOD_BODY = (168, 34, 38)       # 通体血髓艳红
BLOOD_LIT  = (214, 58, 66)       # 棱角透光鲜红
BLOOD_HIGH = (248, 162, 172)     # 晶面向阳粉白冷高光

# 炽火内生脉络（Magma Vein）
MAGMA_GLOW = (255, 92, 42)       # 熔融火脉橙红微光
