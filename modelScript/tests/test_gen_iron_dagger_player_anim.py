import json
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "generators"))

from gen_iron_dagger_player_anim import (
    OUT_BB,
    build_geometry,
    convert_animation,
    main,
)


class TestGenIronDaggerPlayerAnim(unittest.TestCase):
    def test_geometry_and_dagger_attachment(self):
        elements, outliner, gmap, atlas = build_geometry()
        self.assertGreater(len(elements), 40, "应包含玩家身体 + 匕首几何元素")
        self.assertIn("dagger_right_pitch", gmap, "右臂下应挂载匕首 pitch 节点")
        self.assertIn("dagger_right_roll", gmap, "右臂下应挂载匕首 roll 节点")
        self.assertEqual(atlas.size, (128, 128), "图集应为 128x128 组合 Atlas")


if __name__ == "__main__":
    unittest.main()
