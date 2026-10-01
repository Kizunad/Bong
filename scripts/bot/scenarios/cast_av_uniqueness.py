"""R9 P1 D-22 contract anchor：AV 单 owner，保持 inert。"""

DESCRIPTION = "R9 P1 AV 五件套单 owner 契约锚点（待 P3/Wave 2）"
MODULES = ["cast", "combat"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-22", "A-07", "A-08", "A-09", "A-10", "A-11", "A-12", "A-13")


def run(env) -> None:
    """P1 不注册真实招式，唯一 consumer 留给 Wave 2。"""
    del env

