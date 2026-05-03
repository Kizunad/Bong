from __future__ import annotations

import unittest

import numpy as np

from scripts.terrain_gen.spirit_eye_selector import select_spirit_eye_candidates


class SpiritEyeSelectorTest(unittest.TestCase):
    def test_selects_sparse_candidates_in_high_qi_varied_terrain(self) -> None:
        size = 128
        x, z = np.meshgrid(np.arange(size), np.arange(size))
        height = np.full((size, size), 120.0)
        qi_density = np.full((size, size), 0.72)
        feature_mask = np.full((size, size), 0.76)

        mask = select_spirit_eye_candidates(
            height,
            qi_density,
            feature_mask,
            x,
            z,
            density_bias=1.4,
        )

        self.assertEqual(mask.dtype, np.uint8)
        self.assertGreater(int(mask.sum()), 0)
        self.assertLess(int(mask.sum()), size * size // 8)

    def test_rejects_low_qi_wastes_even_when_height_matches(self) -> None:
        size = 64
        height = np.full((size, size), 120.0)
        qi_density = np.full((size, size), 0.05)
        feature_mask = np.full((size, size), 0.95)

        mask = select_spirit_eye_candidates(height, qi_density, feature_mask)

        self.assertEqual(int(mask.sum()), 0)

    def test_blood_valley_allows_rift_feature_candidate_with_lower_qi(self) -> None:
        size = 96
        x, z = np.meshgrid(np.arange(size), np.arange(size))
        height = np.full((size, size), 45.0)
        qi_density = np.full((size, size), 0.26)
        feature_mask = np.full((size, size), 0.88)

        mask = select_spirit_eye_candidates(
            height,
            qi_density,
            feature_mask,
            x,
            z,
            density_bias=1.8,
            blood_valley=True,
        )

        self.assertGreater(int(mask.sum()), 0)


if __name__ == "__main__":
    unittest.main()
