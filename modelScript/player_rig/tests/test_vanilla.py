"""重建的原版几何必须与游戏内实际导出的 ModelPart 顶点逐一一致。"""
import json
import unittest
from pathlib import Path

from modelScript.player_rig import vanilla

DUMP = json.loads((Path(__file__).parent / "vanilla_player_dump.json").read_text())


class VanillaReconstruction(unittest.TestCase):
    def test_every_quad_matches_game_export(self):
        for variant in ("classic", "slim"):
            for name, data in DUMP[variant].items():
                p = vanilla.part(name, variant == "slim")
                mine = vanilla.model_quads(p)
                pivot = data["pivot"]
                self.assertEqual(tuple(pivot), tuple(p.pivot), (variant, name))
                theirs = []
                for quad in data["cuboids"][0]["quads"]:
                    theirs.append(sorted((round(pivot[0] + x, 3), round(24 - (pivot[1] + y), 3),
                                          round(pivot[2] + z, 3), round(u * 64, 3), round(v * 64, 3))
                                         for x, y, z, u, v in quad))
                for quad in mine:
                    key = sorted((round(a, 3), round(b, 3), round(c, 3), round(d, 3), round(e, 3))
                                 for a, b, c, d, e in quad)
                    self.assertIn(key, theirs, (variant, name))


if __name__ == "__main__":
    unittest.main()
