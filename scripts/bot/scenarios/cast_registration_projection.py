"""R9 P1 D-25 contract anchor：registration projection，保持 inert。"""

DESCRIPTION = "R9 P1 registration projection 契约锚点（待 P3/Wave 2）"
MODULES = ["cast", "cultivation"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-25", "A-07", "A-08", "A-09", "A-10", "A-11", "A-12", "A-13", "C-13")


def run(env) -> None:
    """P1 不消费生产 registry；projection 留给全量 activation。"""
    del env

