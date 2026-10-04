"""v2 生物动画采样器的回归测试。

保护的契约：骨煞受击时，五颗外挂小颅都要动起来，而且是随时间摆动的。
PR #2332 的 Kody review 抓到过一次「把相位当成时间传进 wave()」：
``wave(phase)`` 对每颗小颅算出一个与时间无关的常量，相位 0 和 0.5 的两颗恰好恒为 0，
受击时完全不动。这里直接采样作者稿的 hurt 曲线，不经导出器，失败时能直接定位到采样函数。
"""

from __future__ import annotations

import json
import math
import sys
import tempfile
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "creatures" / "fauna_v2"))

import gen_anim  # noqa: E402
import gen_rig  # noqa: E402
from bbmodel_maker.rig.animkit import PoseRig  # noqa: E402

# 受击动作整体乘着包络 sin(πt)。「相位被当时间」时，摆角 = 常量 × 包络，
# 除以包络后是一条水平线；真在摆的话，除完仍然起伏。取包络不为 0 的几个时刻检查。
SAMPLE_TIMES = (0.15, 0.3, 0.45, 0.6, 0.75, 0.9)
MIN_SWING_DEG = 1.0
MIN_SHAPE_SPREAD = 0.2  # 摆角 / 包络 的最大最小差，单位：度 / 包络


class SkullFiendHurtTest(unittest.TestCase):
    def test_every_satellite_skull_swings_over_time_when_hurt(self) -> None:
        _, rig_doc, _ = gen_rig.load_species("skull_fiend_v2")
        with tempfile.TemporaryDirectory(prefix="bong-fauna-v2-") as temp:
            path = Path(temp) / "skull_fiend_v2_rig.bbmodel"
            path.write_text(json.dumps(rig_doc), encoding="utf-8")
            rig = PoseRig(path)

        _, _, hurt = gen_anim.skull_fiend_clips(gen_anim.Poser(rig))["hurt"]
        for bone in gen_anim.SKULL_SATELLITES:
            angles = [hurt(t)[bone].rot[1] for t in SAMPLE_TIMES]
            shape = [angle / math.sin(math.pi * t) for angle, t in zip(angles, SAMPLE_TIMES)]
            with self.subTest(bone=bone):
                self.assertGreater(
                    max(abs(angle) for angle in angles), MIN_SWING_DEG,
                    f"{bone}: 受击时小颅应当摆动，各时刻角度 {[round(a, 2) for a in angles]}",
                )
                self.assertGreater(
                    max(shape) - min(shape), MIN_SHAPE_SPREAD,
                    f"{bone}: 摆角只是包络的常数倍 {[round(s, 2) for s in shape]}，"
                    "说明摆动与时间无关（相位被当成了时间）",
                )


if __name__ == "__main__":
    unittest.main()
