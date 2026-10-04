"""R9 P1 D-21 contract anchor：STOP 语义，保持 inert。"""

DESCRIPTION = "R9 P1 STOP 不终结 gameplay 的契约锚点（待 P2/Wave 2）"
MODULES = ["cast", "combat"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-21", "R-08", "R-09", "R-11", "R-12", "R-13", "R-14", "R-15", "R-16")


def run(env) -> None:
    """P1 只声明 contract anchor，不向真实 producer 注入消息。"""
    del env

