"""R9 P1 D-23 contract anchor：wire identity，保持 inert。"""

DESCRIPTION = "R9 P1 cast wire 完整三元组契约锚点（待 R6/Wave 2）"
MODULES = ["cast", "network"]
DEFAULT_ENABLED = False
CONTRACT_IDS = ("D-23", "P-01", "P-02", "P-03", "P-04", "P-05", "P-06", "P-07", "P-08", "P-09", "P-10", "P-11", "P-12", "P-13", "P-14", "P-15")


def run(env) -> None:
    """P1 不接 transport/bridge；这里只作为清晰的验收锚点。"""
    del env

