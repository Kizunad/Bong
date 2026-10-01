"""R9 P1 D-26 contract anchor：施法 juice identity bridge，保持 inert。"""

DESCRIPTION = "R9 P1 施法 juice 与 identity 对齐契约锚点（待 Wave 2）"
MODULES = ["cast", "combat", "network"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-26", "R-08", "R-09", "R-10", "R-11", "R-12", "R-13", "R-14")


def run(env) -> None:
    """P1 不接 combat juice producer 或 transport。"""
    del env

