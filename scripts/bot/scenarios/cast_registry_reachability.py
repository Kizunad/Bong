"""R9 P1 D-20 contract anchor：注册表可达性，保持 inert。"""

DESCRIPTION = "R9 P1 注册表可达性契约锚点（待 Wave 2 生产接线）"
MODULES = ["cast", "cultivation"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-20", "A-01", "A-02", "A-03", "A-04", "A-05", "A-06", "A-07", "A-08")


def run(env) -> None:
    """P1 不启动 server producer；真实链路留给 Wave 2 atomic activation。"""
    del env

