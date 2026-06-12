import unittest

from scripts.terrain_gen.__main__ import _parse_zone_filter


class TerrainGenCliTest(unittest.TestCase):
    def test_parse_zone_filter_none_means_full_world(self) -> None:
        self.assertIsNone(_parse_zone_filter(None))


    def test_parse_zone_filter_trims_comma_list(self) -> None:
        self.assertEqual(
            _parse_zone_filter(" spawn, qingyun_peaks ,, "),
            {"spawn", "qingyun_peaks"},
        )


    def test_parse_zone_filter_blank_means_full_world(self) -> None:
        self.assertIsNone(_parse_zone_filter(" , "))
