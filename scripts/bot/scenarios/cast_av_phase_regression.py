"""R9 P1 D-24 contract anchor：AV phase 回归，保持 inert。"""

DESCRIPTION = "R9 P1 AV phase 与 terminal 语义契约锚点（待 P2/Wave 2）"
MODULES = ["cast", "combat"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-24", "A-09", "A-11", "A-12", "C-12")


def run(env) -> None:
    """P1 不驱动循环动画或真实 consumer。"""
    del env

